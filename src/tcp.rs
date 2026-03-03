use std::{
    cmp::min,
    io::{self, Write},
    net::Ipv4Addr,
};

use etherparse::{IpNumber, Ipv4Header, Ipv4HeaderSlice, TcpHeader, TcpHeaderSlice};
use tun_tap::Iface;

///                      TCP Connection State Diagram
///
///                              +---------+ ---------\      active OPEN
///                              |  CLOSED |            \    -----------
///                              +---------+<---------\   \   create TCB
///                                |     ^              \   \  snd SYN
///                   passive OPEN |     |   CLOSE        \   \
///                   ------------ |     | ----------       \   \
///                    create TCB  |     | delete TCB         \   \
///                                V     |                      \   \
///                              +---------+            CLOSE    |    \
///                              |  LISTEN |          ---------- |     |
///                              +---------+          delete TCB |     |
///                   rcv SYN      |     |     SEND              |     |
///                  -----------   |     |    -------            |     V
/// +---------+      snd SYN,ACK  /       \   snd SYN          +---------+
/// |         |<-----------------           ------------------>|         |
/// |   SYN   |                    rcv SYN                     |   SYN   |
/// |   RCVD  |<-----------------------------------------------|   SENT  |
/// |         |                    snd ACK                     |         |
/// |         |------------------           -------------------|         |
/// +---------+   rcv ACK of SYN  \       /  rcv SYN,ACK       +---------+
///   |           --------------   |     |   -----------
///   |                  x         |     |     snd ACK
///   |                            V     V
///   |  CLOSE                   +---------+
///   | -------                  |  ESTAB  |
///   | snd FIN                  +---------+
///   |                   CLOSE    |     |    rcv FIN
///   V                  -------   |     |    -------
/// +---------+          snd FIN  /       \   snd ACK          +---------+
/// |  FIN    |<-----------------           ------------------>|  CLOSE  |
/// | WAIT-1  |------------------                              |   WAIT  |
/// +---------+          rcv FIN  \                            +---------+
///   | rcv ACK of FIN   -------   |                            CLOSE  |
///   | --------------   snd ACK   |                           ------- |
///   V        x                   V                           snd FIN V
/// +---------+                  +---------+                   +---------+
/// |FINWAIT-2|                  | CLOSING |                   | LAST-ACK|
/// +---------+                  +---------+                   +---------+
///   |                rcv ACK of FIN |                 rcv ACK of FIN |
///   |  rcv FIN       -------------- |    Timeout=2MSL -------------- |
///   |  -------              x       V    ------------        x       V
///    \ snd ACK                 +---------+delete TCB         +---------+
///     ------------------------>|TIME WAIT|------------------>| CLOSED  |
///                              +---------+                   +---------+
#[derive(Debug, Clone)]
pub enum Connection {
    Closed,
    Listen(ListenerState),
    Active(ActiveSocket),
}

#[derive(Default, Debug, Clone)]
pub struct ListenerState {}

#[derive(Debug, Clone)]
pub struct ActiveSocket {
    state: ActiveState,
    send: SendSequenceSpace,
    recv: RecvSequenceSpace,
    ip: Ipv4Header,
    tcp: TcpHeader,
}

#[derive(Debug, Clone)]
pub enum ActiveState {
    SynSent,
    SynRcvd,
    Estab,
    FinWait1,
    FinWait2,
    TimeWait,
    CloseWait,
    LastAck,
    Closing,
}

/// ```text
/// Send Sequence Space
///
///            1         2          3          4
///       ----------|----------|----------|----------
///              SND.UNA    SND.NXT    SND.UNA
///                                   +SND.WND
///
/// 1 - old sequence numbers which have been acknowledged
/// 2 - sequence numbers of unacknowledged data
/// 3 - sequence numbers allowed for new data transmission
/// 4 - future sequence numbers which are not yet allowed
///
///                   Send Sequence Space
/// ```
#[derive(Debug, Clone)]
struct SendSequenceSpace {
    /// send unacknowledged
    una: u32,
    /// send next
    nxt: u32,
    /// send window
    wnd: u32,
    /// send urgent pointer
    up: bool,
    /// segment sequence number used for last window update
    wl1: u32,
    /// segment acknowledgement number for last window update
    wl2: u32,
    /// initial send sequence number
    iss: u32,
}

/// ```text
/// Receive Sequence Space
///
///                1          2          3
///            ----------|----------|----------
///                   RCV.NXT    RCV.NXT
///                             +RCV.WND
///
/// 1 - old sequence numbers which have been acknowledged
/// 2 - sequence numbers allowed for new reception
/// 3 - future sequence numbers which are not yet allowed
///
///                  Receive Sequence Space
/// ```
#[derive(Debug, Clone)]
struct RecvSequenceSpace {
    /// receive next
    nxt: u32,
    /// receive window
    wnd: u16,
    /// receive urgent pointer
    up: bool,
    /// initial receive sequence number
    irs: u32,
}

impl Connection {
    pub fn accept<'a>(
        nic: &Iface,
        iph: Ipv4HeaderSlice<'a>,
        tcph: TcpHeaderSlice<'a>,
        data: &'a [u8],
    ) -> io::Result<Option<Self>> {
        if !tcph.syn() {
            return Ok(None); // only expected syn packets
        }

        let iss = 0;
        // snd syn,ack -> syn rcvd
        let mut syn_ack = TcpHeader::new(tcph.destination_port(), tcph.source_port(), iss, 10);
        syn_ack.acknowledgment_number = tcph.sequence_number().wrapping_add(1);
        syn_ack.syn = true;
        syn_ack.ack = true;

        let Ok(mut ip) = Ipv4Header::new(
            syn_ack.header_len_u16(),
            64,
            IpNumber::TCP,
            iph.destination(),
            iph.source(),
        ) else {
            eprintln!("ipv4 header new error");
            return Ok(None);
        };
        ip.set_payload_len(syn_ack.header_len() + 0)
            .expect("set payload len fail");

        let mut c = Connection::Active(ActiveSocket {
            state: ActiveState::SynRcvd,
            send: SendSequenceSpace {
                una: iss,
                nxt: iss,
                wnd: 10,
                up: false,
                wl1: 0,
                wl2: 0,
                iss,
            },
            recv: RecvSequenceSpace {
                nxt: tcph.sequence_number().wrapping_add(1),
                wnd: tcph.window_size(),
                up: false,
                irs: tcph.sequence_number(),
            },
            ip,
            tcp: syn_ack,
        });

        c.write_data(nic, &[])?;
        Ok(Some(c))
    }

    pub fn on_packet<'a>(
        &mut self,
        nic: &Iface,
        iph: Ipv4HeaderSlice<'a>,
        tcph: TcpHeaderSlice<'a>,
        data: &'a [u8],
    ) -> io::Result<()> {
        let result: io::Result<()> = match self {
            Connection::Closed => Ok(()),
            Connection::Listen(_) => Ok(()),
            Connection::Active(ActiveSocket {
                state,
                send,
                recv,
                ip,
                tcp,
            }) => {
                // acceptable ack check, remember wrapping
                // SND.UNA < SEG.ACK =< SND.NXT
                let seqn = tcph.sequence_number();
                let mut slen = data.len() as u32;
                if tcph.fin() {
                    slen += 1;
                }
                if tcph.syn() {
                    slen += 1;
                }

                let ackn = tcph.acknowledgment_number();
                let ack_diff = ackn.wrapping_sub(send.una);
                let win_size = send.nxt.wrapping_sub(send.una);

                if ack_diff <= win_size {
                    // all good
                } else {
                    // violated
                    let synchronized =
                        !matches!(state, ActiveState::SynSent | ActiveState::SynRcvd);
                    if !synchronized {
                        // we should send a reset
                        return Self::send_rst_active(nic, ip, tcp, tcph, slen);
                    }
                    // BOGUS: send a empty ack
                    return Ok(());
                }

                let wnd = recv.wnd as u32;

                if slen == 0 {
                    if wnd == 0 {
                        if seqn != recv.nxt {
                            return Ok(());
                        }
                    } else {
                        let offset = seqn.wrapping_sub(recv.nxt);
                        if offset >= wnd {
                            return Ok(());
                        }
                    }
                } else {
                    if wnd == 0 {
                        return Ok(());
                    } else {
                        let seq_end = seqn.wrapping_add(slen).wrapping_sub(1);
                        let start_offset = seqn.wrapping_sub(recv.nxt);
                        let end_offset = seq_end.wrapping_sub(recv.nxt);
                        if !(start_offset < wnd || end_offset < wnd || end_offset < start_offset) {
                            return Ok(());
                        }
                    }
                }

                match state {
                    ActiveState::SynRcvd => {
                        // expect to get a ack for our SYN
                        if !tcph.ack() {
                            return Ok(());
                        }

                        *state = ActiveState::Estab;

                        // now lets terminate the conneection
                        tcp.fin = true;
                        Self::write_data_active(nic, send, recv, ip, tcp, &[])?;
                        *state = ActiveState::FinWait1;

                        unimplemented!()
                    }
                    ActiveState::Estab => {
                        if !tcph.fin() || !data.is_empty() {
                            unimplemented!()
                        }
                        // ack the fin
                        Self::write_data_active(nic, send, recv, ip, tcp, &[])?;
                        *state = ActiveState::CloseWait;
                        unimplemented!()
                    }
                    _ => {
                        unimplemented!()
                    }
                };
            }
        };
        Ok(())
    }

    fn is_synchronized(&self) -> bool {
        match self {
            Connection::Listen(_) => false,
            Connection::Closed => false,
            Connection::Active(s) => {
                !matches!(s.state, ActiveState::SynSent | ActiveState::SynRcvd)
            }
            _ => true,
        }
    }

    fn write_data_active(
        nic: &Iface,
        send: &mut SendSequenceSpace,
        recv: &mut RecvSequenceSpace,
        ip: &mut Ipv4Header,
        tcp: &mut TcpHeader,
        payload: &[u8],
    ) -> io::Result<usize> {
        tcp.sequence_number = send.nxt;
        tcp.acknowledgment_number = recv.nxt;

        let sent = transmit_tcp_packet(nic, ip, tcp, payload)?;

        send.nxt = send.nxt.wrapping_add(payload.len() as u32);
        if tcp.syn {
            send.nxt = send.nxt.wrapping_add(1);
            tcp.syn = false;
        }
        if tcp.fin {
            send.nxt = send.nxt.wrapping_add(1);
            tcp.fin = false;
        }
        Ok(sent)
    }

    fn send_rst_active<'a>(
        nic: &Iface,
        ip: &mut Ipv4Header,
        tcp: &TcpHeader,
        incoming_tcph: TcpHeaderSlice<'a>,
        incoming_slen: u32,
    ) -> io::Result<()> {
        let mut rst_tcp = tcp.clone();

        rst_tcp.rst = true;
        rst_tcp.syn = false;
        rst_tcp.fin = false;

        // BOGUS: if incoming packet has ack, rst_tcp.seq = incoming.ack, else
        // rst_tcp.seq = 0, rst_tcp.ack = incoming.seq + incoming.len()
        if incoming_tcph.ack() {
            rst_tcp.sequence_number = incoming_tcph.acknowledgment_number();
            rst_tcp.ack = false;
        } else {
            rst_tcp.sequence_number = 0;
            rst_tcp.acknowledgment_number =
                incoming_tcph.sequence_number().wrapping_add(incoming_slen);
            rst_tcp.ack = true;
        }

        transmit_tcp_packet(nic, ip, &mut rst_tcp, &[])?;
        Ok(())
    }

    fn write_data(&mut self, nic: &Iface, payload: &[u8]) -> io::Result<usize> {
        match self {
            Connection::Closed => Ok(0),
            Connection::Listen(_) => Ok(0),
            Connection::Active(ActiveSocket {
                send,
                recv,
                ip,
                tcp,
                ..
            }) => Self::write_data_active(nic, send, recv, ip, tcp, payload),
        }
    }

    fn send_rst<'a>(
        &mut self,
        nic: &Iface,
        incoming_tcph: TcpHeaderSlice<'a>,
        incoming_slen: u32,
    ) -> io::Result<()> {
        match self {
            Connection::Closed => unreachable!(),
            Connection::Listen(_) => Ok(()),
            Connection::Active(ActiveSocket { ip, tcp, .. }) => {
                Self::send_rst_active(nic, ip, tcp, incoming_tcph, incoming_slen)
            }
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Quad {
    pub src: (Ipv4Addr, u16),
    pub dst: (Ipv4Addr, u16),
}

fn transmit_tcp_packet(
    nic: &Iface,
    ip: &mut Ipv4Header,
    tcp: &mut TcpHeader,
    payload: &[u8],
) -> io::Result<usize> {
    let mut buf = [0u8; 1500];
    let ip_payload_len = min(
        tcp.header_len() + payload.len(),
        buf.len() - ip.header_len(),
    );
    ip.set_payload_len(ip_payload_len).unwrap();
    tcp.checksum = tcp
        .calc_checksum_ipv4(ip, payload)
        .expect("checksum calc failed");

    let unwritten = {
        let mut unwritten = &mut buf[..];
        ip.write(&mut unwritten)?;
        tcp.write(&mut unwritten)?;
        unwritten.write_all(payload)?;
        unwritten.len()
    };

    let written_len = buf.len() - unwritten;
    nic.send(&buf[..written_len])
}

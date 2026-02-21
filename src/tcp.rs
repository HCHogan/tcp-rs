use std::{io, net::Ipv4Addr};

use etherparse::{IpNumber, Ipv4Header, Ipv4HeaderSlice, TcpHeader, TcpHeaderSlice};
use tun_tap::Iface;

#[derive(Debug)]
pub enum Connection {
    Closed,
    Listen(ListenerState),
    Active(ActiveSocket),
}

#[derive(Default, Debug)]
pub struct ListenerState {}

#[derive(Debug)]
pub struct ActiveSocket {
    state: ActiveState,
    send: SendSequenceSpace,
    recv: RecvSequenceSpace,
}

#[derive(Debug)]
pub enum ActiveState {
    SynSent,
    SynRcvd,
    Estab,
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
#[derive(Debug)]
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
#[derive(Debug)]
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
        let mut c = Connection::Active(ActiveSocket {
            state: ActiveState::SynRcvd,
            send: SendSequenceSpace {
                una: iss,
                nxt: iss + 1,
                wnd: 10,
                up: false,
                wl1: 0,
                wl2: 0,
                iss,
            },
            recv: RecvSequenceSpace {
                nxt: tcph.sequence_number() + 1,
                wnd: tcph.window_size(),
                up: false,
                irs: tcph.sequence_number(),
            },
        });

        // snd syn,ack -> syn rcvd
        let mut syn_ack = TcpHeader::new(tcph.destination_port(), tcph.source_port(), iss, 10);
        syn_ack.acknowledgment_number = tcph.sequence_number() + 1;
        syn_ack.syn = true;
        syn_ack.ack = true;
        let Ok(ip) = Ipv4Header::new(
            syn_ack.header_len_u16(),
            64,
            IpNumber::TCP,
            iph.destination(),
            iph.source(),
        ) else {
            eprintln!("ipv4 header new error");
            return Ok(None);
        };
        let mut buf = [0u8; 1500];
        let unwritten = {
            let mut unwritten = &mut buf[..];
            ip.write(&mut unwritten);
            syn_ack.write(&mut unwritten);
            unwritten.len()
        };
        nic.send(&buf[..buf.len() - unwritten]);
        Ok(Some(c))
    }

    pub fn on_packet<'a>(
        &mut self,
        nic: &Iface,
        iph: Ipv4HeaderSlice<'a>,
        tcph: TcpHeaderSlice<'a>,
        data: &'a [u8],
    ) -> io::Result<usize> {
        Ok(0)
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct Quad {
    pub src: (Ipv4Addr, u16),
    pub dst: (Ipv4Addr, u16),
}

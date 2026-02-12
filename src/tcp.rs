use std::{io, net::Ipv4Addr};

use etherparse::{IpNumber, Ipv4Header, Ipv4HeaderSlice, TcpHeader, TcpHeaderSlice};
use tun_tap::Iface;

#[derive(Debug)]
pub enum TcpState {
    Closed,
    Listen,
    SynRcvd,
    Estab,
}

impl Default for TcpState {
    fn default() -> Self {
        TcpState::Listen
    }
}

impl TcpState {
    pub fn on_packet<'a>(
        &mut self,
        nic: &Iface,
        iph: Ipv4HeaderSlice<'a>,
        tcph: TcpHeaderSlice<'a>,
        data: &'a [u8],
    ) -> io::Result<usize> {
        match *self {
            TcpState::Closed => {
                return Ok(0);
            }

            TcpState::Listen => {
                if !tcph.syn() {
                    return Ok(0); // only expected syn packets
                }

                // snd syn,ack -> syn rcvd
                let mut syn_ack = TcpHeader::new(
                    tcph.destination_port(),
                    tcph.source_port(),
                    unimplemented!(),
                    unimplemented!(),
                );
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
                    return Ok(0);
                };
                let mut buf = [0u8; 1500];
                let unwritten = {
                    let mut unwritten = &mut buf[..];
                    ip.write(&mut unwritten);
                    syn_ack.write(&mut unwritten);
                    unwritten.len()
                };
                nic.send(&buf[..buf.len() - unwritten]);
            }

            _ => {
                return Ok(0);
            }
        }

        let src = iph.source_addr();
        let dst = iph.destination_addr();
        let src_port = tcph.source_port();
        let dst_port = tcph.destination_port();
        let len = data.len();
        eprintln!(
            "{}:{} -> {}:{} {}b of tcp",
            src, dst, src_port, dst_port, len
        );
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct Quad {
    pub src: (Ipv4Addr, u16),
    pub dst: (Ipv4Addr, u16),
}

use std::net::Ipv4Addr;

use etherparse::{Ipv4HeaderSlice, TcpHeaderSlice};

#[derive(Debug)]
pub struct TcpState {}

impl Default for TcpState {
    fn default() -> Self {
        TcpState {}
    }
}

impl TcpState {
    pub fn on_packet<'a>(
        &mut self,
        iph: Ipv4HeaderSlice<'a>,
        tcph: TcpHeaderSlice<'a>,
        data: &'a [u8],
    ) {
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

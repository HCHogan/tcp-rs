mod refs;
mod tcp;

use etherparse::{IpNumber, Ipv4HeaderSlice, TcpHeaderSlice};
use std::collections::HashMap;
use std::io;
use tun_tap::{Iface, Mode};

use tcp::{Quad, TcpState};

fn main() -> io::Result<()> {
    let nic = Iface::new("tun0", Mode::Tun)?;
    let mut buf = [0u8; 1504];
    let mut connections: HashMap<Quad, TcpState> = Default::default();
    loop {
        let nbytes = nic.recv(&mut buf[..])?;

        let flags = u16::from_be_bytes([buf[0], buf[1]]);
        let eth_proto = u16::from_be_bytes([buf[2], buf[3]]);

        if eth_proto != 0x0800 {
            continue;
        }

        match Ipv4HeaderSlice::from_slice(&buf[4..nbytes]) {
            Ok(iph) => {
                let src = iph.source_addr();
                let dst = iph.destination_addr();
                let proto = iph.protocol();
                if proto != IpNumber::TCP {
                    continue;
                }
                let Ok(len) = iph.payload_len() else {
                    eprintln!("len error!");
                    continue;
                };
                match TcpHeaderSlice::from_slice(&buf[4 + iph.slice().len()..nbytes]) {
                    Ok(tcph) => {
                        let datai = 4 + iph.slice().len() + tcph.slice().len();
                        let src_port = tcph.source_port();
                        let dst_port = tcph.destination_port();

                        connections
                            .entry(Quad {
                                src: (src, src_port),
                                dst: (dst, dst_port),
                            })
                            .or_default()
                            .on_packet(iph, tcph, &buf[datai..nbytes]);
                    }

                    Err(e) => {
                        eprintln!("ignoring weird tcp packet: {e}");
                    }
                }
            }
            Err(e) => {
                eprintln!("ignoring weird packet: {}", e);
            }
        }
    }
}

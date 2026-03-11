mod kioto;
mod rc;
mod refs;
mod t1;
mod tcp;

use etherparse::{IpNumber, Ipv4HeaderSlice, TcpHeaderSlice};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::io;
use tun_tap::{Iface, Mode};

use crate::tcp::{Connection, Quad};

fn main() -> io::Result<()> {
    let mut nic = Iface::without_packet_info("tun0", Mode::Tun)?;
    let mut buf = [0u8; 1500];
    let mut connections: HashMap<Quad, Connection> = Default::default();
    loop {
        let nbytes = nic.recv(&mut buf[..])?;

        // let _flags = u16::from_be_bytes([buf[0], buf[1]]);
        // let eth_proto = u16::from_be_bytes([buf[2], buf[3]]);
        //
        // if eth_proto != 0x0800 {
        //     continue;
        // }

        let Ok(iph) = Ipv4HeaderSlice::from_slice(&buf[..nbytes]) else {
            eprintln!("ignoring weird packet");
            continue;
        };

        let src = iph.source_addr();
        let dst = iph.destination_addr();
        let proto = iph.protocol();
        if proto != IpNumber::TCP {
            continue;
        }
        let Ok(_len) = iph.payload_len() else {
            eprintln!("len error!");
            continue;
        };

        let Ok(tcph) = TcpHeaderSlice::from_slice(&buf[iph.slice().len()..nbytes]) else {
            eprintln!("ignoring weird packet");
            continue;
        };

        let datai = iph.slice().len() + tcph.slice().len();
        let src_port = tcph.source_port();
        let dst_port = tcph.destination_port();

        match connections.entry(Quad {
            src: (src, src_port),
            dst: (dst, dst_port),
        }) {
            Entry::Occupied(mut c) => {
                c.get_mut()
                    .on_packet(&mut nic, iph, tcph, &buf[datai..nbytes])?;
            }
            Entry::Vacant(e) => {
                if let Some(c) = Connection::accept(&mut nic, iph, tcph, &buf[datai..nbytes])? {
                    e.insert(c);
                }
            }
        }
    }
}

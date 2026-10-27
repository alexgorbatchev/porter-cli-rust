use anyhow::{Context, Result, bail};
use netlink_packet_core::{
    NLM_F_DUMP, NLM_F_DUMP_INTR, NLM_F_REQUEST, NetlinkHeader, NetlinkMessage, NetlinkPayload,
};
use netlink_packet_sock_diag::{
    SockDiagMessage,
    constants::{AF_INET, AF_INET6, IPPROTO_TCP, TCP_LISTEN},
    inet::{ExtensionFlags, InetRequest, SocketId, StateFlags},
};
use netlink_sys::{Socket, SocketAddr, protocols::NETLINK_SOCK_DIAG};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

pub type Listeners = BTreeMap<u64, BTreeSet<u16>>;

pub fn apply_datagram(bytes: &[u8], sequence: u32, listeners: &mut Listeners) -> Result<bool> {
    let mut offset = 0;
    while offset < bytes.len() {
        let packet = NetlinkMessage::<SockDiagMessage>::deserialize(&bytes[offset..])
            .context("decode socket diagnostics")?;
        let length = packet.header.length as usize;
        if length < 16 || length > bytes.len() - offset {
            bail!("invalid socket diagnostics frame length");
        }
        offset += (length + 3) & !3;
        if packet.header.sequence_number != sequence {
            continue;
        }
        if packet.header.flags & NLM_F_DUMP_INTR != 0 {
            bail!("socket diagnostics dump interrupted; refresh again");
        }
        match packet.payload {
            NetlinkPayload::Done(done) => {
                if done.code != 0 {
                    return Err(std::io::Error::from_raw_os_error(
                        done.code.saturating_abs(),
                    ))
                    .context("socket diagnostics dump failed");
                }
                return Ok(true);
            }
            NetlinkPayload::Error(error) if error.code.is_some() => {
                return Err(error.to_io()).context("socket diagnostics request failed");
            }
            NetlinkPayload::Overrun(_) => bail!("socket diagnostics response overrun"),
            NetlinkPayload::InnerMessage(SockDiagMessage::InetResponse(response)) => {
                let header = response.header;
                if matches!(header.family, AF_INET | AF_INET6) && header.state == TCP_LISTEN {
                    listeners
                        .entry(u64::from(header.inode))
                        .or_default()
                        .insert(header.socket_id.source_port);
                }
            }
            _ => {}
        }
    }
    Ok(false)
}

pub fn listeners() -> Result<Listeners> {
    let mut socket = Socket::new(NETLINK_SOCK_DIAG).context("open socket diagnostics")?;
    socket.bind_auto().context("bind socket diagnostics")?;
    socket
        .connect(&SocketAddr::new(0, 0))
        .context("connect socket diagnostics")?;
    rustix::net::sockopt::set_socket_timeout(
        &socket,
        rustix::net::sockopt::Timeout::Recv,
        Some(Duration::from_secs(2)),
    )?;
    let mut listeners = Listeners::new();
    for (index, family) in [AF_INET, AF_INET6].into_iter().enumerate() {
        let sequence = index as u32 + 1;
        let mut header = NetlinkHeader::default();
        header.flags = NLM_F_REQUEST | NLM_F_DUMP;
        header.sequence_number = sequence;
        let mut id = if family == AF_INET {
            SocketId::new_v4()
        } else {
            SocketId::new_v6()
        };
        id.cookie = [0xff; 8];
        let mut packet = NetlinkMessage::new(
            header,
            SockDiagMessage::InetRequest(InetRequest {
                family,
                protocol: IPPROTO_TCP,
                extensions: ExtensionFlags::empty(),
                states: StateFlags::LISTEN,
                socket_id: id,
            })
            .into(),
        );
        packet.finalize();
        let mut bytes = vec![0; packet.buffer_len()];
        packet.serialize(&mut bytes);
        let length = socket
            .send(&bytes, 0)
            .context("send socket diagnostics request")?;
        if length != bytes.len() {
            bail!("incomplete socket diagnostics request");
        }
        loop {
            let (bytes, sender) = match socket.recv_from_full() {
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                result => result.context("receive socket diagnostics")?,
            };
            if sender.port_number() != 0 {
                continue;
            }
            if apply_datagram(&bytes, sequence, &mut listeners)? {
                break;
            }
        }
    }
    Ok(listeners)
}

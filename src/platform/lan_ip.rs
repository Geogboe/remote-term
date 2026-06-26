use std::net::IpAddr;

pub fn primary_lan_ip() -> Option<IpAddr> {
    local_ip_address::local_ip().ok().filter(|ip| match ip {
        IpAddr::V4(ip) => !ip.is_loopback() && !ip.is_link_local(),
        IpAddr::V6(ip) => !ip.is_loopback() && !ip.is_unspecified(),
    })
}

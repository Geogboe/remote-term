use std::net::IpAddr;

pub fn primary_lan_ip() -> Option<IpAddr> {
    local_ip_address::local_ip().ok().filter(|ip| match ip {
        IpAddr::V4(ip) => !ip.is_loopback() && !ip.is_link_local(),
        IpAddr::V6(ip) => !ip.is_loopback() && !ip.is_unspecified(),
    })
}

pub fn terminal_url(ip: IpAddr, port: u16, token: &str) -> String {
    match ip {
        IpAddr::V4(ip) => format!("http://{ip}:{port}/t/{token}"),
        IpAddr::V6(ip) => format!("http://[{ip}]:{port}/t/{token}"),
    }
}

pub fn local_access_ip(bind_ip: IpAddr) -> IpAddr {
    if bind_ip.is_unspecified() {
        match bind_ip {
            IpAddr::V4(_) => IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            IpAddr::V6(_) => IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
        }
    } else {
        bind_ip
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv6_terminal_urls_use_bracketed_hosts() {
        let ip = "2001:db8::1".parse().unwrap();
        assert_eq!(
            terminal_url(ip, 7843, "secret"),
            "http://[2001:db8::1]:7843/t/secret"
        );
    }

    #[test]
    fn wildcard_binds_advertise_the_matching_loopback_family() {
        assert_eq!(
            local_access_ip("0.0.0.0".parse().unwrap()),
            "127.0.0.1".parse::<IpAddr>().unwrap()
        );
        assert_eq!(
            local_access_ip("::".parse().unwrap()),
            "::1".parse::<IpAddr>().unwrap()
        );
        assert_eq!(
            local_access_ip("127.0.0.2".parse().unwrap()),
            "127.0.0.2".parse::<IpAddr>().unwrap()
        );
    }
}

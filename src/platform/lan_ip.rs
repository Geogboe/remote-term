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
}

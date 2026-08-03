fn safe_private_dns_name(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 63
        && !host.contains('.')
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && host
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && host
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
}

fn local_endpoint_has_supported_private_shape(endpoint: &url::Url) -> bool {
    let Some(host) = endpoint.host_str() else {
        return false;
    };
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(address)) => address.is_private() || address.is_loopback(),
        Ok(std::net::IpAddr::V6(address)) => address.is_unique_local() || address.is_loopback(),
        Err(_) => safe_private_dns_name(&host.to_ascii_lowercase()),
    }
}

fn parse_private_cidr(value: &str) -> KernelResult<LocalProviderNetworkEntry> {
    let (address, prefix) = value.split_once('/').ok_or(
        TrpgError::InvalidConfiguration("local_provider_allowlist_invalid"),
    )?;
    let address = address.parse::<std::net::IpAddr>().map_err(|_| {
        TrpgError::InvalidConfiguration("local_provider_allowlist_invalid")
    })?;
    let prefix = prefix.parse::<u8>().map_err(|_| {
        TrpgError::InvalidConfiguration("local_provider_allowlist_invalid")
    })?;
    let (canonical_network, final_address, private) = match address {
        std::net::IpAddr::V4(address) if prefix <= 32 => {
            let mask = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - prefix)
            };
            let network = u32::from(address) & mask;
            let final_address = network | !mask;
            let network_address = std::net::Ipv4Addr::from(network);
            let final_address_value = std::net::Ipv4Addr::from(final_address);
            (
                std::net::IpAddr::V4(network_address),
                std::net::IpAddr::V4(final_address_value),
                (network_address.is_private() && final_address_value.is_private())
                    || (network_address.is_loopback() && final_address_value.is_loopback()),
            )
        }
        std::net::IpAddr::V6(address) if prefix <= 128 => {
            let mask = if prefix == 0 {
                0
            } else {
                u128::MAX << (128 - prefix)
            };
            let network = u128::from(address) & mask;
            let final_address = network | !mask;
            let network_address = std::net::Ipv6Addr::from(network);
            let final_address_value = std::net::Ipv6Addr::from(final_address);
            (
                std::net::IpAddr::V6(network_address),
                std::net::IpAddr::V6(final_address_value),
                (network_address.is_unique_local() && final_address_value.is_unique_local())
                    || (network_address.is_loopback() && final_address_value.is_loopback()),
            )
        }
        _ => {
            return Err(TrpgError::InvalidConfiguration(
                "local_provider_allowlist_invalid",
            ))
        }
    };
    if !private || canonical_network != address || !same_address_family(canonical_network, final_address)
    {
        return Err(TrpgError::InvalidConfiguration(
            "local_provider_allowlist_invalid",
        ));
    }
    Ok(LocalProviderNetworkEntry::Cidr {
        network: canonical_network,
        prefix,
    })
}

fn same_address_family(left: std::net::IpAddr, right: std::net::IpAddr) -> bool {
    matches!(
        (left, right),
        (std::net::IpAddr::V4(_), std::net::IpAddr::V4(_))
            | (std::net::IpAddr::V6(_), std::net::IpAddr::V6(_))
    )
}

fn address_in_cidr(
    address: std::net::IpAddr,
    network: std::net::IpAddr,
    prefix: u8,
) -> bool {
    match (address, network) {
        (std::net::IpAddr::V4(address), std::net::IpAddr::V4(network)) => {
            let mask = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - prefix)
            };
            u32::from(address) & mask == u32::from(network)
        }
        (std::net::IpAddr::V6(address), std::net::IpAddr::V6(network)) => {
            let mask = if prefix == 0 {
                0
            } else {
                u128::MAX << (128 - prefix)
            };
            u128::from(address) & mask == u128::from(network)
        }
        _ => false,
    }
}

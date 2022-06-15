use anyhow::Result;
use ethabi::{Event, EventParam, ParamType};

/// returns the erc20 transfer event
/// "event Transfer(address indexed from, address indexed to, uint value)"
#[allow(dead_code)]
pub fn erc20_transfer() -> Result<Event> {
    let params = vec![
        EventParam {
            name: "from".to_string(),
            kind: ParamType::Address,
            indexed: true,
        },
        EventParam {
            name: "to".to_string(),
            kind: ParamType::Address,
            indexed: true,
        },
        EventParam {
            name: "value".to_string(),
            kind: ParamType::Uint(256),
            indexed: false,
        },
    ];
    let event = Event {
        name: "Transfer".to_string(),
        inputs: params,
        anonymous: false,
    };
    Ok(event)
}

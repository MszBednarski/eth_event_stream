use ethabi::{Event, EventParam, ParamType};
use nom::branch::alt;
use nom::bytes::complete::take_while;
use nom::sequence::tuple;
use nom::{
    bytes::complete::{tag, take_until},
    character::is_alphabetic,
    multi::many0,
    sequence::delimited,
    IResult,
};

fn name_parser(input: &[u8]) -> IResult<&[u8], &[u8]> {
    let p = take_until("(");
    let (input, name) = p(input)?;
    if name.starts_with(b"event") {
        panic!("`event` keyword should be ommited");
    }
    Ok((input, name))
}

fn param_parser(input: &[u8]) -> IResult<&[u8], (&[u8], &[u8], &[u8])> {
    // let inputb = input.as_bytes();
    let mut p = tuple((
        many0(tag(" ")),
        // take the type
        take_while(is_alphabetic),
        many0(tag(" ")),
        // maybe take indexed
        alt((tag("indexed"), tag(""))),
        many0(tag(" ")),
        // take the name
        take_while(is_alphabetic),
        // get trailling whitespace
        many0(tag(" ")),
    ));
    let (input, (_, type_, _, indexed, _, name, _)) = p(input)?;
    Ok((input, (type_, indexed, name)))
}

/// the solidity declaration string will be consumed in the process
/// Example declarations:
/// Transfer(address indexed from, address indexed to, uint value)
/// event Start(uint start, uint middle, uint end) anonymous;
fn parse_event_declaration(
    input: &[u8],
) -> IResult<&[u8], (&[u8], Vec<(&[u8], &[u8], &[u8])>, &[u8])> {
    let (input, name) = name_parser(input)?;
    let previous_param_parser = tuple((param_parser, tag(",")));
    let multi_param_parser = tuple((many0(previous_param_parser), param_parser));
    let mut parser_empty = delimited(tag("("), many0(tag(" ")), tag(")"));
    let res = parser_empty(input.clone());
    let mut anonymous_parser = tuple((many0(tag(" ")), alt((tag("anonymous"), tag("")))));
    // if event with no parameters
    if res.is_ok() {
        let (input, _) = res?;
        let (input, (_, anonymous)) = anonymous_parser(input)?;
        return Ok((input, (name, Vec::new(), anonymous)));
    }
    // if event with parameters
    let mut parser = delimited(tag("("), multi_param_parser, tag(")"));
    // (Vec<((&[u8], &[u8], &[u8]), &[u8])>, (&[u8], &[u8], &[u8]))
    let (input, (prev_params, last_param)): (
        &[u8],
        (Vec<((&[u8], &[u8], &[u8]), &[u8])>, (&[u8], &[u8], &[u8])),
    ) = parser(input)?;
    let mut params: Vec<(&[u8], &[u8], &[u8])> = prev_params
        .iter()
        .map(|a| match a {
            (vals, _) => vals.to_owned(),
        })
        .collect();
    params.push(last_param);
    let (input, (_, anonymous)) = anonymous_parser(input)?;
    Ok((input, (name, params, anonymous)))
}

pub fn event_from_declaration(declaration: String) -> anyhow::Result<Event> {
    let decb = declaration.into_bytes();
    let (input, (name, params, anonymous)) = parse_event_declaration(decb.as_slice()).unwrap();
    if input.len() > 0 {
        return Err(anyhow::anyhow!(
            "The input event declaration has trailing data [{}]",
            std::str::from_utf8(input)?
        ));
    }

    Ok(Event {
        name: std::str::from_utf8(name)?.to_string(),
        anonymous: anonymous == b"anonymous",
        inputs: params
            .iter()
            .map(|p| {
                (
                    std::str::from_utf8(p.0).unwrap(),
                    p.1 == b"indexed",
                    std::str::from_utf8(p.2).unwrap().to_string(),
                )
            })
            .map(|p| match p {
                (_type, indexed, name) => EventParam {
                    name,
                    kind: match _type {
                        "address" => ParamType::Address,
                        "uint" => ParamType::Uint(256),
                        "uint256" => ParamType::Uint(256),
                        "bool" => ParamType::Bool,
                        _ => todo!(),
                    },
                    indexed,
                },
            })
            .collect(),
    })
}

#[cfg(test)]
mod test {
    use crate::events::{event_from_declaration, parse_event_declaration};
    use ethabi::{Event, EventParam, ParamType};

    use super::{name_parser, param_parser};

    #[test]
    fn test_parse_event_declaration() -> anyhow::Result<()> {
        let decl = "Transfer(address indexed from, address indexed to, uint value)".as_bytes();
        assert_eq!(
            name_parser(decl),
            Ok((
                "(address indexed from, address indexed to, uint value)".as_bytes(),
                "Transfer".as_bytes()
            ))
        );
        assert_eq!(
            param_parser("address indexed from, ".as_bytes()),
            Ok((
                ", ".as_bytes(),
                (
                    "address".as_bytes(),
                    "indexed".as_bytes(),
                    "from".as_bytes()
                )
            ))
        );
        assert_eq!(
            parse_event_declaration(decl),
            Ok((
                "".as_bytes(),
                (
                    "Transfer".as_bytes(),
                    vec![
                        (
                            "address".as_bytes(),
                            "indexed".as_bytes(),
                            "from".as_bytes()
                        ),
                        ("address".as_bytes(), "indexed".as_bytes(), "to".as_bytes()),
                        ("uint".as_bytes(), "".as_bytes(), "value".as_bytes()),
                    ],
                    "".as_bytes()
                )
            ))
        );
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
        let erc20_transfer_event = Event {
            name: "Transfer".to_string(),
            inputs: params,
            anonymous: false,
        };
        assert_eq!(
            event_from_declaration(
                "Transfer(address indexed from, address indexed to, uint value)".to_string()
            )?,
            erc20_transfer_event
        );
        Ok(())
    }
}

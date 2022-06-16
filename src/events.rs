use ethabi::{Event, EventParam, ParamType};
use nom::branch::alt;
use nom::bytes::complete::take_while;
use nom::multi::many_m_n;
use nom::sequence::tuple;
use nom::{
    bytes::complete::{is_not, tag, take_until, take_while_m_n},
    character::is_alphabetic,
    multi::many0,
    sequence::separated_pair,
    IResult,
};

fn name_parser(input: &str) -> IResult<&str, &str> {
    let mut p = tuple((tag("event"), many_m_n(1, 10, tag(" ")), take_until("(")));
    let (input, (_, _, name)) = p(input)?;
    Ok((input, name))
}

fn _param_parser(input: &[u8]) -> IResult<&[u8], (&[u8], &[u8], &[u8])> {
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

fn param_parser(input: &'static str) -> anyhow::Result<(&str, (&str, &str, &str))> {
    let (input, (t, i, n)) = _param_parser(input.as_bytes())?;
    Ok((
        std::str::from_utf8(input)?,
        (
            std::str::from_utf8(t)?,
            std::str::from_utf8(i)?,
            std::str::from_utf8(n)?,
        ),
    ))
}

/// the solidity declaration string will be consumed in the process
/// Example declarations:
/// event Transfer(address indexed from, address indexed to, uint value)
/// event Start(uint start, uint middle, uint end) anonymous;
// fn parse_event_declaration(input: &[u8]) -> IResult<&[u8], Vec<(&[u8], &[u8], &[u8])>> {
//     let (input, name) = name_parser(input)?;
//     let mut previous_param_parser = tuple((last_param_parser, tag(",")));
//     let mut multi_param_parser = tuple((many0(previous_param_parser), last_param_parser));
//     let mut param_parser = delimited(tag("("), multi_param_parser, tag(")"));
//     let (input, (prev, last)) = param_parser(input)?;
//     let prev = prev
//         .iter()
//         .map(|p| match p {
//             ((_, type_, _, indexed, _, name, _), _) => {
//                 (type_.to_owned(), indexed.to_owned(), name.to_owned())
//             }
//         })
//         .collect();
//     Ok((input, (prev)))
//     // // get a bunch of params or just spaces
//     // let params_parser = alt((multi_param_parser, space_parser));
//     // // get the entire signature
//     // let sig_parser = delimited(tag("("), params_parser, tag(")"));
//     // // get the entire event declaration
//     // let parser = tuple((
//     //     tag("event"),
//     //     many_m_n(1, 10, tag(" ")),
//     //     take_until("("),
//     //     sig_parser,
//     // ));
//     // let (input, res) = parser(input)?;
// }

/// returns the erc20 transfer event

#[allow(dead_code)]
pub fn erc20_transfer() -> anyhow::Result<Event> {
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

#[cfg(test)]
mod test {
    use super::{name_parser, param_parser};

    #[test]
    fn test_parse_event_declaration() -> anyhow::Result<()> {
        let decl = "event Transfer(address indexed from, address indexed to, uint value)";
        assert_eq!(
            name_parser(decl),
            Ok((
                "(address indexed from, address indexed to, uint value)",
                "Transfer"
            ))
        );
        assert_eq!(
            param_parser("address indexed from, ")?,
            (", ", ("address", "indexed", "from"))
        );
        Ok(())
    }
}

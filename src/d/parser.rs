//! [The original grammar can be found
//! here](https://github.com/opendtrace/opendtrace/blob/master/lib/libdtrace/common/dt_grammar.y). This
//! parser re-implements the `provider_definition` rule (with its
//! children, `provider_probe_list`, `provider_probe`, `function`
//! etc.).

use super::ast::*;
use nom::{
    bytes::complete::{tag, take_until, take_while},
    character::{complete::char, is_alphanumeric},
    combinator::map,
    error::{ParseError, VerboseError},
    multi::{many0, separated_list0},
    sequence::{delimited, preceded, terminated, tuple},
    IResult,
};

// Canonicalization of a `$parser`, i.e. remove the whitespace before it.
macro_rules! canon {
    ($parser:expr) => {
        preceded(ws, $parser)
    };
}

/// Parse whitespaces.
fn ws<'i, E: ParseError<&'i str>>(input: &'i str) -> IResult<&'i str, &'i str, E> {
    let chars = "\t\r\n ";

    take_while(move |c| chars.contains(c))(input)
}

/// Parse a name.
fn name<'i, E: ParseError<&'i str>>(input: &'i str) -> IResult<&'i str, &'i str, E> {
    take_while(|c| is_alphanumeric(c as u8) || c == '-' || c == '_')(input)
}

/// Parse a type. That's super generic. It doesn't validate anything specifically.
///
/// Note: This is incomplete for the moment. See the
/// `parameter_type_list` from the official grammar (see module's
/// documentation).
fn ty<'i, E: ParseError<&'i str>>(input: &'i str) -> IResult<&'i str, &'i str, E> {
    let chars = ",)";

    take_while(move |c| !chars.contains(c))(input)
}

/// Parse a `probe`.
fn probe<'i, E: ParseError<&'i str>>(input: &'i str) -> IResult<&'i str, Probe, E> {
    map(
        tuple((
            preceded(tag("probe"), canon!(name)),
            delimited(
                canon!(char('(')),
                separated_list0(char(','), canon!(ty)),
                canon!(terminated(char(')'), canon!(char(';')))),
            ),
        )),
        |(name, arguments)| Probe {
            name: name.into(),
            arguments: arguments
                .iter()
                .filter_map(|argument| {
                    let argument = argument.trim();

                    if argument.is_empty() {
                        None
                    } else {
                        Some(argument.to_string())
                    }
                })
                .collect(),
        },
    )(input)
}

/// Parse a `provider`.
fn provider<'i, E: ParseError<&'i str>>(input: &'i str) -> IResult<&'i str, Provider, E> {
    map(
        tuple((
            preceded(tag("provider"), canon!(name)),
            delimited(
                canon!(char('{')),
                many0(canon!(probe)),
                canon!(terminated(char('}'), canon!(char(';')))),
            ),
        )),
        |(name, probes)| Provider {
            name: name.into(),
            probes,
        },
    )(input)
}

/// Parse a script. It collects only the `provider` blocks, nothing else.
fn script<'i, E: ParseError<&'i str>>(mut input: &'i str) -> IResult<&'i str, Script, E> {
    let mut script = Script { providers: vec![] };

    loop {
        match take_until::<_, _, E>("provider")(input) {
            Ok((input_next, _)) => {
                let (input_next, output) = provider(input_next)?;

                script.providers.push(output);

                input = input_next;
            }

            _ => return Ok(("", script)),
        }
    }
}

/// Parse a `.d` file and return a [`Script`] value.
pub fn parse<'i>(input: &'i str) -> Result<Script, String> {
    match script::<VerboseError<&'i str>>(input) {
        Ok((_, output)) => Ok(output),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        assert_eq!(name::<()>("foobar"), Ok(("", "foobar")));
    }

    #[test]
    fn test_type() {
        assert_eq!(ty::<()>("char"), Ok(("", "char")));
        assert_eq!(ty::<()>("short"), Ok(("", "short")));
        assert_eq!(ty::<()>("int"), Ok(("", "int")));
        assert_eq!(ty::<()>("long"), Ok(("", "long")));
        assert_eq!(ty::<()>("long long"), Ok(("", "long long")));
        assert_eq!(ty::<()>("uint32_t"), Ok(("", "uint32_t")));
        assert_eq!(ty::<()>("char *"), Ok(("", "char *")));
        assert_eq!(ty::<()>("foo bar *,"), Ok((",", "foo bar *")));
        assert_eq!(ty::<()>("foo bar *)"), Ok((")", "foo bar *")));
    }

    #[test]
    fn test_probe_with_zero_argument() {
        assert_eq!(
            probe::<()>("probe abc();"),
            Ok((
                "",
                Probe {
                    name: "abc".to_string(),
                    arguments: vec![],
                }
            ))
        );
    }

    #[test]
    fn test_probe_with_one_argument() {
        assert_eq!(
            probe::<()>("probe abc ( char * ) ;"),
            Ok((
                "",
                Probe {
                    name: "abc".to_string(),
                    arguments: vec!["char *".to_string()],
                }
            ))
        );
    }

    #[test]
    fn test_probe_with_many_argument() {
        assert_eq!(
            probe::<()>("probe abc ( char *, uint8_t ) ;"),
            Ok((
                "",
                Probe {
                    name: "abc".to_string(),
                    arguments: vec!["char *".to_string(), "uint8_t".to_string()],
                }
            ))
        );
    }

    #[test]
    fn test_empty_provider() {
        assert_eq!(
            provider::<()>("provider foobar { } ;"),
            Ok((
                "",
                Provider {
                    name: "foobar".to_string(),
                    probes: vec![]
                }
            ))
        );
    }

    #[test]
    fn test_provider() {
        assert_eq!(
            provider::<()>(
                "provider foobar {
                     probe abc(char*, int);
                     probe def(string);
                 };"
            ),
            Ok((
                "",
                Provider {
                    name: "foobar".to_string(),
                    probes: vec![
                        Probe {
                            name: "abc".to_string(),
                            arguments: vec!["char*".to_string(), "int".to_string()],
                        },
                        Probe {
                            name: "def".to_string(),
                            arguments: vec!["string".to_string()],
                        }
                    ]
                }
            ))
        );
    }

    #[test]
    fn test_script() {
        assert_eq!(
            script::<()>(
                "provider foobar {
                     probe abc(char*, int);
                     probe def(string);
                 };

                 provider hopla {
                     probe xyz();
                 };"
            ),
            Ok((
                "",
                Script {
                    providers: vec![
                        Provider {
                            name: "foobar".to_string(),
                            probes: vec![
                                Probe {
                                    name: "abc".to_string(),
                                    arguments: vec!["char*".to_string(), "int".to_string()],
                                },
                                Probe {
                                    name: "def".to_string(),
                                    arguments: vec!["string".to_string()],
                                }
                            ]
                        },
                        Provider {
                            name: "hopla".to_string(),
                            probes: vec![Probe {
                                name: "xyz".to_string(),
                                arguments: vec![]
                            }],
                        },
                    ]
                }
            ))
        );
    }
}

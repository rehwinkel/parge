use std::{collections::HashSet, fs::File, io::Read, path::Path};

use color_eyre::eyre::{bail, ensure, Result};
use nom::{
    branch::alt,
    bytes::complete::{escaped, tag, take_while, take_while1, take_while_m_n},
    character::complete::{newline, one_of, satisfy, space0, space1},
    combinator::{map, opt},
    error::ParseError,
    multi::{many0, separated_list0, separated_list1},
    sequence::tuple,
    IResult,
};
use smol_str::SmolStr;

#[derive(Debug)]
pub enum Element {
    Rule {
        var: Option<SmolStr>,
        name: SmolStr,
    },
    Set {
        chars: Vec<char>,
        ranges: Vec<(char, char)>,
    },
    NegatedSet {
        chars: Vec<char>,
        ranges: Vec<(char, char)>,
    },
    Literal {
        lit: SmolStr,
    },
    OneOrMore {
        inner: Box<Element>,
    },
    ZeroOrMore {
        inner: Box<Element>,
    },
    Optional {
        inner: Box<Element>,
    },
    Alternatives {
        subelems: Vec<Element>,
    },
    Group {
        subelems: Vec<Element>,
    },
}

#[derive(Debug)]
pub struct Rule {
    pub is_terminal: bool,
    pub export: bool,
    pub name: SmolStr,
    pub element: Element,
    pub constructor_name: Option<SmolStr>,
    pub constructor_vars: Option<Vec<SmolStr>>,
}

fn parse_set<'src>(src: &'src str) -> IResult<&'src str, Element> {
    let (src, _) = tag("[")(src)?;
    let (src, negated) = opt(tag("^"))(src)?;
    let negated = negated.is_some();
    enum CharOrRange {
        Char(char),
        Range((char, char)),
    }
    let (src, char_or_range) = many0(alt((
        map(tag("\\]"), |_| CharOrRange::Char(']')),
        map(tag("\\\\"), |_| CharOrRange::Char('\\')),
        map(tag("\\-"), |_| CharOrRange::Char('-')),
        map(
            tuple((
                satisfy(|c: char| c != ']'),
                tag("-"),
                satisfy(|c: char| c != ']'),
            )),
            |(a, _, b)| CharOrRange::Range((a, b)),
        ),
        map(satisfy(|c: char| c != ']'), |c| CharOrRange::Char(c)),
    )))(src)?;
    let (src, _) = tag("]")(src)?;
    let mut chars = Vec::new();
    let mut ranges = Vec::new();
    for cor in char_or_range {
        match cor {
            CharOrRange::Char(c) => chars.push(c),
            CharOrRange::Range(c) => ranges.push(c),
        }
    }
    if negated {
        Ok((src, Element::NegatedSet { chars, ranges }))
    } else {
        Ok((src, Element::Set { chars, ranges }))
    }
}

fn parse_literal<'src>(src: &'src str) -> IResult<&'src str, Element> {
    let (src, _) = tag("\"")(src)?;
    let (src, contents) = escaped(take_while1(|c: char| c != '"'), '\\', tag("\""))(src)?;
    let (src, _) = tag("\"")(src)?;
    Ok((
        src,
        Element::Literal {
            lit: SmolStr::new(contents),
        },
    ))
}

fn parse_repetition<'src>(src: &'src str) -> IResult<&'src str, Element> {
    let (src, base) = parse_group(src)?;
    let inner = Box::new(base);
    let (src, kind) = one_of("+*?")(src)?;
    match kind {
        '+' => Ok((src, Element::OneOrMore { inner })),
        '*' => Ok((src, Element::ZeroOrMore { inner })),
        '?' => Ok((src, Element::Optional { inner })),
        _ => Err(nom::Err::Error(nom::error::Error::from_error_kind(
            src,
            nom::error::ErrorKind::MapRes,
        ))),
    }
}

fn parse_repetition_no_rule<'src>(src: &'src str) -> IResult<&'src str, Element> {
    let (src, base) = parse_group_no_rule(src)?;
    let inner = Box::new(base);
    let (src, kind) = one_of("+*?")(src)?;
    match kind {
        '+' => Ok((src, Element::OneOrMore { inner })),
        '*' => Ok((src, Element::ZeroOrMore { inner })),
        '?' => Ok((src, Element::Optional { inner })),
        _ => Err(nom::Err::Error(nom::error::Error::from_error_kind(
            src,
            nom::error::ErrorKind::MapRes,
        ))),
    }
}

fn parse_group<'src>(src: &'src str) -> IResult<&'src str, Element> {
    let (src, _) = tag("(")(src)?;
    let (src, _) = space0(src)?;
    let (src, mut elements) = separated_list1(space1, parse_element)(src)?;
    let (src, _) = space0(src)?;
    let (src, _) = tag(")")(src)?;
    if elements.len() == 1 {
        Ok((src, elements.remove(0)))
    } else {
        Ok((src, Element::Group { subelems: elements }))
    }
}

fn parse_alternatives<'src>(src: &'src str) -> IResult<&'src str, Element> {
    let (src, _) = tag("(")(src)?;
    let (src, _) = space0(src)?;
    let (src, mut elements) =
        separated_list1(tuple((space0, tag("|"), space0)), parse_element)(src)?;
    let (src, _) = space0(src)?;
    let (src, _) = tag(")")(src)?;
    if elements.len() == 1 {
        Ok((src, elements.remove(0)))
    } else {
        Ok((src, Element::Alternatives { subelems: elements }))
    }
}

fn parse_group_no_rule<'src>(src: &'src str) -> IResult<&'src str, Element> {
    let (src, _) = tag("(")(src)?;
    let (src, _) = space0(src)?;
    let (src, mut elements) = separated_list1(space1, parse_element_no_rule)(src)?;
    let (src, _) = space0(src)?;
    let (src, _) = tag(")")(src)?;
    if elements.len() == 1 {
        Ok((src, elements.remove(0)))
    } else {
        Ok((src, Element::Group { subelems: elements }))
    }
}

fn parse_alternatives_no_rule<'src>(src: &'src str) -> IResult<&'src str, Element> {
    let (src, _) = tag("(")(src)?;
    let (src, _) = space0(src)?;
    let (src, mut elements) =
        separated_list1(tuple((space0, tag("|"), space0)), parse_element_no_rule)(src)?;
    let (src, _) = space0(src)?;
    let (src, _) = tag(")")(src)?;
    if elements.len() == 1 {
        Ok((src, elements.remove(0)))
    } else {
        Ok((src, Element::Alternatives { subelems: elements }))
    }
}

fn parse_element_rule<'src>(src: &'src str) -> IResult<&'src str, Element> {
    let (src, var_opt) = opt(tuple((parse_name, tag(":"))))(src)?;
    let var = var_opt.map(|(var, _)| var);
    let (src, name) = parse_name(src)?;
    Ok((src, Element::Rule { var, name }))
}

fn parse_element<'src>(src: &'src str) -> IResult<&'src str, Element> {
    alt((
        parse_repetition,
        parse_literal,
        parse_set,
        parse_element_rule,
        parse_group,
        parse_alternatives,
    ))(src)
}

fn parse_element_no_rule<'src>(src: &'src str) -> IResult<&'src str, Element> {
    alt((
        parse_repetition_no_rule,
        parse_literal,
        parse_set,
        parse_group_no_rule,
        parse_alternatives_no_rule,
    ))(src)
}

fn parse_token<'src>(src: &'src str) -> IResult<&'src str, Rule> {
    let (src, _) = tag("token")(src)?;
    let (src, _) = space1(src)?;
    let (src, name) = parse_name(src)?;
    let (src, _) = space0(src)?;
    let (src, _) = tag("=")(src)?;
    let (src, _) = space0(src)?;
    let (src, elements) = separated_list1(space1, parse_element_no_rule)(src)?;
    let (src, _) = tag(";")(src)?;
    Ok((
        src,
        Rule {
            export: false,
            is_terminal: true,
            name,
            element: Element::Group { subelems: elements },
            constructor_name: None,
            constructor_vars: None,
        },
    ))
}

fn parse_constructor<'src>(src: &'src str) -> IResult<&'src str, (SmolStr, Vec<SmolStr>)> {
    let (src, type_name) = parse_name(src)?;
    let (src, _) = tag("(")(src)?;
    let (src, vars) = separated_list0(tuple((space0, tag(","), space0)), parse_name)(src)?;
    let (src, _) = tag(")")(src)?;
    Ok((src, (type_name, vars)))
}

fn parse_name<'src>(src: &'src str) -> IResult<&'src str, SmolStr> {
    let (src, name_fc) = take_while_m_n(1, 1, |c: char| c.is_alphabetic())(src)?;
    let (src, name) = take_while(|c: char| c.is_alphanumeric() || c == '_')(src)?;
    let name = SmolStr::new(format!("{}{}", name_fc, name));
    Ok((src, name))
}

fn parse_nonterminal<'src>(src: &'src str) -> IResult<&'src str, Rule> {
    let (src, _) = tag("nonterm")(src)?;
    let (src, _) = space1(src)?;
    let (src, name) = parse_name(src)?;
    let (src, _) = space0(src)?;
    let (src, _) = tag("=")(src)?;
    let (src, _) = space0(src)?;
    let (src, elements) = separated_list1(space1, parse_element)(src)?;
    let (src, _) = space0(src)?;
    let (src, _) = tag("->")(src)?;
    let (src, _) = space0(src)?;
    let (src, (type_name, vars)) = parse_constructor(src)?;
    let (src, _) = tag(";")(src)?;
    Ok((
        src,
        Rule {
            export: false,
            is_terminal: false,
            name,
            element: Element::Group { subelems: elements },
            constructor_name: Some(type_name),
            constructor_vars: Some(vars),
        },
    ))
}

fn parse_rule<'src>(src: &'src str) -> IResult<&'src str, Rule> {
    let (src, export) = opt(tag("export "))(src)?;
    let (src, mut rule) = alt((parse_token, parse_nonterminal))(src)?;
    rule.export = export.is_some();
    Ok((src, rule))
}

fn parse_rules<'src>(src: &'src str) -> IResult<&'src str, Vec<Rule>> {
    let (src, rules) = separated_list1(newline, parse_rule)(src)?;
    let (src, _) = opt(newline)(src)?;
    Ok((src, rules))
}

pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Vec<Rule>> {
    let mut rule_file = File::open(path)?;
    let mut src = String::new();
    rule_file.read_to_string(&mut src)?;
    match parse_rules(&src) {
        Ok((rest, rules)) => {
            ensure!(
                rest.is_empty(),
                "Failed to parse whole file, remainder was: {:?}",
                rest
            );
            let mut rule_names = HashSet::new();
            for rule in &rules {
                rule_names.insert(&rule.name);
            }
            ensure!(rule_names.len() == rules.len(), "Rule names aren't unique");
            Ok(rules)
        }
        Err(nom::Err::Error(nom::error::Error { input, code })) => {
            bail!(
                "Error '{:?}' while parsing with remaining input: {:?}",
                code,
                input
            )
        }
        _ => bail!("Unexpected error while parsing"),
    }
}

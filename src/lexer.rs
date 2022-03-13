use smol_str::SmolStr;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fmt::Write,
    os::linux::raw,
};

use crate::rules::{Element, Rule};

pub struct Lexer {}

#[derive(Debug)]
struct State {
    accepting: Option<SmolStr>,
}

#[derive(Debug)]
enum EpsilonConnection {
    Epsilon(usize, usize),
    Connection((u32, u32), usize, usize),
}

impl EpsilonConnection {
    fn get_a(&self) -> usize {
        match self {
            EpsilonConnection::Connection(_, a, _) => *a,
            EpsilonConnection::Epsilon(a, _) => *a,
        }
    }
    fn get_b(&self) -> usize {
        match self {
            EpsilonConnection::Connection(_, _, b) => *b,
            EpsilonConnection::Epsilon(_, b) => *b,
        }
    }
}

#[derive(Debug)]
enum Connection {
    Char(char, usize, usize),
    Range((char, char), usize, usize),
}

struct NFA {
    states: Vec<State>,
    entry: usize,
    connections: Vec<EpsilonConnection>,
}

struct DFA {
    states: Vec<State>,
    entry: usize,
    connections: Vec<Connection>,
}

impl DFA {
    fn new() -> Self {
        let mut states = Vec::new();
        let entry = states.len();
        states.push(State { accepting: None });
        DFA {
            states,
            entry,
            connections: Vec::new(),
        }
    }
}

impl NFA {
    fn new() -> Self {
        let mut states = Vec::new();
        let entry = states.len();
        states.push(State { accepting: None });
        NFA {
            states,
            entry,
            connections: Vec::new(),
        }
    }

    fn as_dot_string(&self) -> Result<String, std::fmt::Error> {
        let mut out = String::new();

        write!(out, "digraph {{")?;
        for connection in &self.connections {
            let (a, b, label) = match connection {
                EpsilonConnection::Connection(r, a, b) => {
                    if r.0 == r.1 {
                        (a, b, format!("{:?}", r.0))
                    } else {
                        (a, b, format!("{:?}-{:?}", r.0, r.1))
                    }
                }
                EpsilonConnection::Epsilon(a, b) => (a, b, format!("EPS")),
            };
            write!(
                out,
                "{} -> {} [label=\"{}\"] ",
                if let State {
                    accepting: Some(name),
                } = self.states.get(a.clone()).unwrap()
                {
                    format!("{}{}", name.to_string(), a)
                } else {
                    format!("S{}", a)
                },
                if let State {
                    accepting: Some(name),
                } = self.states.get(b.clone()).unwrap()
                {
                    format!("{}{}", name.to_string(), b)
                } else {
                    format!("S{}", b)
                },
                label
            )?;
        }
        write!(out, "}}")?;
        Ok(out)
    }

    fn add(&mut self, state: State) -> usize {
        let l = self.states.len();
        self.states.push(state);
        l
    }

    fn add_empty(&mut self) -> usize {
        self.add(State { accepting: None })
    }

    fn connect_range(&mut self, start: usize, end: usize, range: (u32, u32)) {
        self.connections
            .push(EpsilonConnection::Connection(range, start, end))
    }

    fn connect_epsilon(&mut self, start: usize, end: usize) {
        self.connections
            .push(EpsilonConnection::Epsilon(start, end))
    }
}

fn connect_element(nfa: &mut NFA, alphabet: &Vec<(u32, u32)>, element: &Element) -> (usize, usize) {
    match element {
        Element::Group { subelems } => {
            assert!(subelems.len() > 0);
            if subelems.len() == 1 {
                connect_element(nfa, alphabet, &subelems[0])
            } else {
                let first = &subelems[0];
                let last = subelems.last().unwrap();
                let (entry, mut o) = connect_element(nfa, alphabet, first);
                for i in 1..subelems.len() - 1 {
                    let elem = &subelems[i];
                    let (i, o2) = connect_element(nfa, alphabet, elem);
                    nfa.connect_epsilon(o, i);
                    o = o2;
                }
                let (i, exit) = connect_element(nfa, alphabet, last);
                nfa.connect_epsilon(o, i);
                (entry, exit)
            }
        }
        Element::Set { chars, ranges } => {
            let entry = nfa.add_empty();
            let exit = nfa.add_empty();
            let mut connections = HashSet::new();
            for c in chars {
                connections.insert((*c as u32, *c as u32));
            }
            for range in ranges {
                let start_index = alphabet.iter().position(|r| r.0 == range.0 as u32).unwrap();
                let end_index = alphabet.iter().position(|r| r.1 == range.1 as u32).unwrap() + 1;
                for range in &alphabet[start_index..end_index] {
                    connections.insert((range.0, range.1));
                }
            }
            for connection in connections {
                nfa.connect_range(entry, exit, connection);
            }
            (entry, exit)
        }
        Element::Alternatives { subelems } => {
            let entry = nfa.add_empty();
            let exit = nfa.add_empty();
            for elem in subelems {
                let (elem_start, elem_end) = connect_element(nfa, alphabet, elem);
                nfa.connect_epsilon(entry, elem_start);
                nfa.connect_epsilon(elem_end, exit);
            }
            (entry, exit)
        }
        Element::OneOrMore { inner } => {
            let (entry, exit) = connect_element(nfa, alphabet, &inner);
            nfa.connect_epsilon(exit, entry);
            (entry, exit)
        }
        Element::ZeroOrMore { inner } => {
            let (entry, exit) = connect_element(nfa, alphabet, &inner);
            nfa.connect_epsilon(exit, entry);
            nfa.connect_epsilon(entry, exit);
            (entry, exit)
        }
        Element::Rule { .. } => panic!(),
        Element::NegatedSet { chars, ranges } => {
            let entry = nfa.add_empty();
            let exit = nfa.add_empty();
            let mut connections: HashSet<(u32, u32)> =
                alphabet.iter().map(|r| (r.0, r.1)).collect();
            for c in chars {
                connections.remove(&(*c as u32, *c as u32));
            }
            for range in ranges {
                let start_index = alphabet.iter().position(|r| r.0 == range.0 as u32).unwrap();
                let end_index = alphabet.iter().position(|r| r.1 == range.1 as u32).unwrap() + 1;
                for range in &alphabet[start_index..end_index] {
                    connections.remove(&(range.0, range.1));
                }
            }
            for connection in connections {
                nfa.connect_range(entry, exit, connection);
            }
            (entry, exit)
        }
        Element::Literal { lit } => {
            let start = nfa.add_empty();
            let mut chars = lit.chars();

            let first = chars.next().unwrap();
            let mut prev = start;
            let mut end = nfa.add_empty();
            nfa.connect_range(prev, end, (first as u32, first as u32));
            prev = end;

            for c in chars {
                end = nfa.add_empty();
                nfa.connect_range(prev, end, (c as u32, c as u32));
                prev = end;
            }
            (start, end)
        }
        Element::Optional { inner } => {
            let (entry, exit) = connect_element(nfa, alphabet, &inner);
            nfa.connect_epsilon(entry, exit);
            (entry, exit)
        }
    }
}

fn get_ranges_from_element(element: &Element, raw_ranges: &mut BTreeSet<(char, char)>) {
    match element {
        Element::Set { chars, ranges } => {
            for c in chars {
                raw_ranges.insert((*c, *c));
            }
            for r in ranges {
                raw_ranges.insert((r.0, r.1));
            }
        }
        Element::NegatedSet { chars, ranges } => {
            for c in chars {
                raw_ranges.insert((*c, *c));
            }
            for r in ranges {
                raw_ranges.insert((r.0, r.1));
            }
        }
        Element::Literal { lit } => {
            for c in lit.chars() {
                raw_ranges.insert((c, c));
            }
        }
        Element::OneOrMore { inner } => get_ranges_from_element(inner, raw_ranges),
        Element::ZeroOrMore { inner } => get_ranges_from_element(inner, raw_ranges),
        Element::Optional { inner } => get_ranges_from_element(inner, raw_ranges),
        Element::Alternatives { subelems } => {
            for elem in subelems {
                get_ranges_from_element(elem, raw_ranges)
            }
        }
        Element::Group { subelems } => {
            for elem in subelems {
                get_ranges_from_element(elem, raw_ranges)
            }
        }
        _ => panic!(),
    }
}

fn construct_alphabet<'a, I>(rules: I) -> Vec<(u32, u32)>
where
    I: Iterator<Item = &'a Rule>,
{
    let mut raw_ranges = BTreeSet::new();
    for rule in rules {
        get_ranges_from_element(&rule.element, &mut raw_ranges);
    }
    let range_points: Vec<u32> = raw_ranges
        .iter()
        .flat_map(|(a, b)| [*a, *b].into_iter().map(|c| c as u32))
        .collect::<BTreeSet<u32>>()
        .into_iter()
        .collect();
    let mut ranges = BTreeSet::new();
    let mut prev = 0u32;
    for point in range_points {
        ranges.insert((prev, prev));
        if prev + 1 < point - 1 {
            ranges.insert((prev + 1, point - 1));
        }
        ranges.insert((point, point));
        prev = point;
    }
    if prev + 1 <= char::MAX as u32 {
        ranges.insert((prev + 1, char::MAX as u32));
    }
    ranges.into_iter().collect()
}

fn construct_nfa<'a, I>(rules: I, alphabet: &Vec<(u32, u32)>) -> NFA
where
    I: Iterator<Item = &'a Rule>,
{
    let mut nfa = NFA::new();
    for rule in rules {
        let exit = nfa.add(State {
            accepting: Some(rule.name.clone()),
        });
        let (elem_entry, elem_exit) = connect_element(&mut nfa, alphabet, &rule.element);
        nfa.connect_epsilon(nfa.entry, elem_entry);
        nfa.connect_epsilon(elem_exit, exit);
    }
    nfa
}

fn epsilon_closure(nfa: &NFA, connected: &mut BTreeSet<usize>) {
    for connection in &nfa.connections {
        if let EpsilonConnection::Epsilon(a, b) = connection {
            if connected.contains(a) {
                if !connected.contains(b) {
                    connected.insert(*b);
                    epsilon_closure(nfa, connected);
                }
            }
        }
    }
}

fn powerset_construction(
    nfa: &NFA,
    start_closure: &BTreeSet<usize>,
    powersets: &mut HashSet<BTreeSet<usize>>,
    alphabet: &Vec<(u32, u32)>,
) -> DFA {
    let dfa = DFA::new();
    for arange in alphabet {
        let mut transition_closure = BTreeSet::new();
        for connection in &nfa.connections {
            if start_closure.contains(&connection.get_a()) {
                match connection {
                    EpsilonConnection::Epsilon(..) => (),
                    &EpsilonConnection::Connection(range, _, b) => {
                        if arange == &range {
                            transition_closure.insert(b);
                        }
                    }
                }
            }
        }
        epsilon_closure(nfa, &mut transition_closure);
        println!(
            "{:?} {:?} -> {:?}",
            arange, start_closure, &transition_closure
        );
        if !powersets.contains(&transition_closure) {
            powersets.insert(transition_closure.clone());
            powerset_construction(nfa, &transition_closure, powersets, alphabet);
        }
        // todo: add connection (arange.0, arange.1)
    }
    dfa
}

impl Lexer {
    pub fn from_rules(rules: &Vec<Rule>) -> Self {
        let alphabet = construct_alphabet(rules.iter().filter(|rule| rule.is_terminal));
        let nfa = construct_nfa(rules.iter().filter(|rule| rule.is_terminal), &alphabet);
        //println!("{:?}", nfa.states);
        //println!("{:?}", nfa.connections);
        //println!("{}", nfa.as_dot_string().unwrap());
        let mut powersets = HashSet::new();
        let mut closure = BTreeSet::new();
        closure.insert(nfa.entry);
        epsilon_closure(&nfa, &mut closure);
        powersets.insert(closure.clone());
        let dfa = powerset_construction(&nfa, &closure, &mut powersets, &alphabet);
        println!("{:?}", powersets);
        Lexer {}
    }
}

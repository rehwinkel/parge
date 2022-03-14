use color_eyre::{eyre::ensure, Result};
use smol_str::SmolStr;
use std::collections::{BTreeSet, HashSet};

use crate::rules::{Element, Rule};

pub struct Lexer {
    dfa: DFA,
}

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
}

#[derive(Debug)]
struct Connection {
    range: (u32, u32),
    start: usize,
    end: usize,
}

struct NFA {
    states: Vec<State>,
    entry: usize,
    connections: Vec<EpsilonConnection>,
}

struct DFA {
    states: Vec<State>,
    connections: Vec<Connection>,
}

impl DFA {
    fn new() -> Self {
        DFA {
            states: Vec::new(),
            connections: Vec::new(),
        }
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
        self.connections.push(Connection { range, start, end })
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
        if prev + 1 <= point - 1 {
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
    start_closure: usize,
    powersets: &mut Vec<BTreeSet<usize>>,
    connections: &mut Vec<Connection>,
    alphabet: &Vec<(u32, u32)>,
) {
    for arange in alphabet {
        let mut transition_closure = BTreeSet::new();
        for connection in &nfa.connections {
            if powersets[start_closure].contains(&connection.get_a()) {
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
        let pos = if let Some(pos) = powersets.iter().position(|c| c == &transition_closure) {
            pos
        } else {
            let pos = powersets.len();
            powersets.push(transition_closure);
            powerset_construction(nfa, pos, powersets, connections, alphabet);
            pos
        };
        connections.push(Connection {
            range: (arange.0, arange.1),
            start: start_closure,
            end: pos,
        });
    }
}

impl Lexer {
    pub fn from_rules(rules: &Vec<Rule>) -> Result<Self> {
        let alphabet = construct_alphabet(rules.iter().filter(|rule| rule.is_terminal));
        let nfa = construct_nfa(rules.iter().filter(|rule| rule.is_terminal), &alphabet);
        let mut powersets = Vec::new();
        let mut connections = Vec::new();
        let mut closure = BTreeSet::new();
        closure.insert(nfa.entry);
        epsilon_closure(&nfa, &mut closure);
        powersets.push(closure);
        powerset_construction(&nfa, 0, &mut powersets, &mut connections, &alphabet);
        let mut dfa = DFA::new();
        for ps in powersets {
            if ps.is_empty() {
                dfa.add(State {
                    accepting: Some(SmolStr::from("_TRAP")),
                });
                continue;
            }
            let mut acceptions = Vec::new();
            for i in ps {
                if let Some(accept) = &nfa.states[i].accepting {
                    acceptions.push(accept);
                }
            }
            ensure!(
                acceptions.len() < 2,
                "Accepting state must accept exactly one rule"
            );
            if acceptions.is_empty() {
                dfa.add_empty();
            } else {
                dfa.add(State {
                    accepting: Some(acceptions[0].clone()),
                });
            }
        }
        for c in connections {
            dfa.connect_range(c.start, c.end, c.range);
        }
        Ok(Lexer { dfa })
    }

    pub fn get_states(&self) -> Vec<Option<&SmolStr>> {
        self.dfa
            .states
            .iter()
            .map(|s| s.accepting.as_ref())
            .collect()
    }

    pub fn get_connections(&self, start: usize) -> Vec<(u32, u32, usize)> {
        self.dfa
            .connections
            .iter()
            .filter(|&c| c.start == start)
            .map(|c| (c.range.0, c.range.1, c.end))
            .collect()
    }
}

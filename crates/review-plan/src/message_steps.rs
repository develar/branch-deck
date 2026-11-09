//! The steps of the message of one commit (openspec/specs/branch/review-tour/spec.md, "Steps of the Message"): a body whose
//! parts name the code they tell is the author's story of the change, so its parts are the steps, in its order, each
//! with the concerns its code names. No git process: the plan holds the units, the concerns and the joins that made
//! them.
//!
//! - **The tellings** of a body (`Telling`): a labelled section of no verification (a label or a heading and its
//!   blocks), else each paragraph, list or table of a run with the blocks written right under it (`CommitBody::tight`:
//!   a list under its paragraph); the lead, a Look closely, the code alone and a verification section are none, but a
//!   `Tests:` section tells the tests.
//! - **The seeds** of a telling: the parts of the concerns (a unit in a file) that its code names resolve to, against no
//!   step (`story::unit_mentions`): a declaration its parts, a file its parts in that file. A part two tellings name is
//!   the seed of one: a labelled section before a paragraph, a paragraph before the tests, then the one that names it
//!   most, then the earlier one.
//! - **When**: two tellings of the story or more keep seeds. Otherwise the concerns stay the steps: a body of one
//!   paragraph that names code, or two sections that name the same code (a cause and its fix), the first keeping it.
//! - **A concern** goes whole to the one telling that seeds it. A concern two tellings seed is cut by its own evidence
//!   (`Concern::links`, a use the change adds nearer than one it keeps): each part goes to the nearest seed, the
//!   telling of the better rank and the earlier one on a tie; a part no evidence reaches goes to the telling that
//!   seeds most of it. No evidence is added: a concern no telling names stays a step of its own after the story, and
//!   one of test code alone joins the step of the tests. What no evidence joined (the rest) goes part by part: a part
//!   a telling names to it, a test part to the tests, the others stay the rest.
//! - **A step** of a telling shows the parts it got; its title is the label of its section, else the first sentence of
//!   its paragraph when it is short (`MAX_TITLE`), its backticks aside, which its opening tells in place, else the
//!   first code name of the paragraph that resolves. Its id is the fingerprint of its first part, so a reword keeps it.
//!   Its opening tells its part unfolded (`story::story_commits`).
//! - **Where the body is told**: each telling in its step, a Look closely in the step of the telling before it; the
//!   rest of the body as of the concerns (`story::told_in`).

use crate::concerns::{Concern, ConcernName, unquoted};
use crate::files::test_subject;
use crate::steps::{CommitRole, ReviewStep, StepStop, StepsInput, TitleSource, split_message, step_commit};
use crate::story::{Mention, MentionTarget, unit_mentions};
use crate::units::digest;
use domain_types::commit_body::{CommitBody, CommitBodyBlock, CommitBodyPartKind, LabelKind};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet};
use std::ops::Range;
use sync_utils::commit_body::commit_body;

/// The longest first sentence that titles its step, its backticks aside: the budget of a subject
const MAX_TITLE: usize = 100;

/// The words before a period that end no sentence
const ABBREVIATIONS: [&str; 7] = ["e.g", "i.e", "vs", "etc", "cf", "approx", "no"];

/// The steps of the message of one commit, and where its body is told
pub(crate) struct MessageSteps {
  pub steps: Vec<ReviewStep>,
  /// The step whose opening tells each block of the body, by its place in `steps`; `None` where the concerns' rule
  /// places it (`story::told_in`)
  pub told: Vec<Option<u32>>,
  /// The first blocks of the sections whose label or heading is the title of their step (`StoryPart::titled`)
  pub titled: BTreeSet<u32>,
}

/// How strongly a telling claims what it names: a labelled section, a paragraph or a list, the tests
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
  Section,
  Paragraph,
  Tests,
}

/// A part of the body that may tell a step
struct Telling {
  blocks: Range<usize>,
  rank: Rank,
  /// Its title and how it titles the step
  title: Option<(String, TitleSource)>,
}

/// A part of a concern by its places: the concern in `StepsInput::concerns`, the part in `Concern::parts`
type Place = (usize, usize);

/// The steps of the message of the one commit of `input`; `None` when its body tells no two steps by the code it names
pub(crate) fn message_steps(input: &StepsInput<'_>) -> Option<MessageSteps> {
  let [(_, message)] = input.commits else { return None };
  let body = commit_body(split_message(message).1);
  let mentions = unit_mentions(input, &body);
  let mut tellings = tellings_of(&body);
  // a paragraph whose first sentence is long is titled by the first name it resolves, set as code
  for telling in tellings.iter_mut().filter(|telling| telling.title.is_none()) {
    let blocks = telling.blocks.clone();
    telling.title = mentions
      .iter()
      .find(|mention| blocks.contains(&(mention.block as usize)) && mention.target.is_some())
      .map(|mention| (unquoted(&mention.text).to_string(), TitleSource::Name));
  }
  let seeds = seeds_of(input, &mentions, &tellings);
  let story: BTreeSet<usize> = seeds.values().copied().filter(|&telling| tellings[telling].rank != Rank::Tests).collect();
  if story.len() < 2 {
    return None;
  }
  let tests = tellings.iter().position(|telling| telling.rank == Rank::Tests);
  let owner = assign(input, &tellings, &seeds, tests);
  // the steps: the tellings that got a part, in the order of the body; then the concerns no telling names; the rest last
  let mut held: BTreeMap<usize, Vec<Place>> = BTreeMap::new();
  let mut left: BTreeMap<usize, Vec<Place>> = BTreeMap::new();
  for (concern, of) in input.concerns.iter().enumerate() {
    for part in 0..of.parts.len() {
      match owner.get(&(concern, part)) {
        Some(&telling) => held.entry(telling).or_default().push((concern, part)),
        None => left.entry(concern).or_default().push((concern, part)),
      }
    }
  }
  let starts = vec![step_commit(input, 0, CommitRole::Starts)];
  let stop = |(concern, part): Place| {
    let part = &input.concerns[concern].parts[part];
    StepStop::new(part.unit.clone(), part.path.clone(), part.blocks.clone())
  };
  let mut steps: Vec<ReviewStep> = Vec::new();
  let mut step_of: HashMap<usize, u32> = HashMap::new();
  for (&telling, places) in &held {
    let first = &input.concerns[places[0].0].parts[places[0].1];
    let mut step = ReviewStep::new(digest(["message", first.unit.as_str(), first.path.as_str()]));
    step.commits.clone_from(&starts);
    step.stops = places.iter().copied().map(stop).collect();
    if let Some((title, source)) = &tellings[telling].title {
      step.title = Some(title.clone());
      step.title_source = *source;
    }
    step_of.insert(telling, u32::try_from(steps.len()).unwrap_or(u32::MAX));
    steps.push(step);
  }
  let mut rest: Option<ReviewStep> = None;
  for (concern, places) in left {
    let of: &Concern = &input.concerns[concern];
    let mut step = ReviewStep::new(of.id.clone());
    step.commits.clone_from(&starts);
    step.concern = Some(of.id.clone());
    step.stops = places.into_iter().map(stop).collect();
    if of.name == ConcernName::Rest {
      rest = Some(step);
    } else {
      steps.push(step);
    }
  }
  steps.extend(rest);
  let mut told = vec![None; body.blocks.len()];
  let mut titled = BTreeSet::new();
  for (place, telling) in tellings.iter().enumerate() {
    let Some(&step) = step_of.get(&place) else { continue };
    told[telling.blocks.clone()].fill(Some(step));
    if telling.title.as_ref().is_some_and(|(_, source)| *source == TitleSource::Label) {
      titled.insert(u32::try_from(telling.blocks.start).unwrap_or(u32::MAX));
    }
  }
  // a Look closely asks about the telling before it: told in its step
  for part in body.parts.iter().filter(|part| part.tone == LabelKind::LookClosely) {
    let before = tellings
      .iter()
      .enumerate()
      .filter(|(place, telling)| telling.blocks.end <= part.start as usize && step_of.contains_key(place))
      .max_by_key(|(_, telling)| telling.blocks.end);
    if let Some((place, _)) = before {
      told[part.start as usize..part.end as usize].fill(Some(step_of[&place]));
    }
  }
  Some(MessageSteps { steps, told, titled })
}

/// The tellings of `body` (`Telling`), in its order
fn tellings_of(body: &CommitBody) -> Vec<Telling> {
  let mut tellings = Vec::new();
  for part in &body.parts {
    let range = part.start as usize..part.end as usize;
    let head = &body.blocks[range.start];
    match (part.kind, part.tone) {
      (CommitBodyPartKind::Section, LabelKind::Plain) => {
        let title = match head {
          CommitBodyBlock::Label { text, .. } | CommitBodyBlock::Heading { text, .. } => Some((text.clone(), TitleSource::Label)),
          _ => None,
        };
        tellings.push(Telling {
          blocks: range,
          rank: Rank::Section,
          title,
        });
      }
      (CommitBodyPartKind::Section, LabelKind::Verification) if is_tests(head) => {
        let CommitBodyBlock::Label { text, .. } = head else { continue };
        tellings.push(Telling {
          blocks: range,
          rank: Rank::Tests,
          title: Some((text.clone(), TitleSource::Label)),
        });
      }
      (CommitBodyPartKind::Run, _) => {
        // a paragraph, a list or a table with the blocks written right under it (a list under its paragraph): one
        // telling; a table, of no sentence, titled by the first name it resolves
        let mut place = range.start;
        while place < range.end {
          let mut end = place + 1;
          while end < range.end && body.tight.get(end).copied().unwrap_or(false) {
            end += 1;
          }
          let first_line = match &body.blocks[place] {
            CommitBodyBlock::Paragraph { lines } => Some(lines.first()),
            CommitBodyBlock::List { items, .. } => Some(items.first()),
            CommitBodyBlock::Table { .. } => Some(None),
            _ => None,
          };
          if let Some(line) = first_line {
            tellings.push(Telling {
              blocks: place..end,
              rank: Rank::Paragraph,
              title: line.and_then(|line| first_sentence(line)).map(|sentence| (sentence, TitleSource::Sentence)),
            });
          }
          place = end;
        }
      }
      _ => {}
    }
  }
  tellings
}

/// Whether a label of verification heads the tests: "Tests", "Test", "The tests"
fn is_tests(head: &CommitBodyBlock) -> bool {
  let CommitBodyBlock::Label { text, .. } = head else { return false };
  let lower = text.to_lowercase();
  let head = lower.split(',').next().unwrap_or_default().trim();
  let head = head.strip_prefix("the ").unwrap_or(head);
  head == "test" || head == "tests" || head.starts_with("tests ") || head.starts_with("test ")
}

/// The first sentence of `line`, its backticks aside, when it ends within the line and is at most `MAX_TITLE`
/// characters: up to a `.`, `?` or `!` outside backticks before a space or the end of the line, not after an
/// abbreviation (`e.g.`), a single letter or a digit (`v2.`)
fn first_sentence(line: &str) -> Option<String> {
  let chars: Vec<char> = line.chars().collect();
  let mut quoted = false;
  let mut end = None;
  for (at, &c) in chars.iter().enumerate() {
    if c == '`' {
      quoted = !quoted;
      continue;
    }
    if quoted || !matches!(c, '.' | '?' | '!') {
      continue;
    }
    let at_break = chars.get(at + 1).is_none_or(|next| next.is_whitespace());
    let word: String = chars[..at].iter().rev().take_while(|c| !c.is_whitespace()).collect::<Vec<_>>().into_iter().rev().collect();
    let word = word.trim_start_matches(['(', '"', '\'']).to_lowercase();
    let abbreviated = c == '.' && (ABBREVIATIONS.contains(&word.as_str()) || word.chars().count() == 1 || word.ends_with(|c: char| c.is_ascii_digit()));
    if at_break && !abbreviated {
      end = Some(at + 1);
      break;
    }
  }
  let sentence: String = chars[..end?].iter().filter(|&&c| c != '`').collect();
  let sentence = sentence.trim().to_string();
  (sentence.chars().count() <= MAX_TITLE).then_some(sentence)
}

/// The seed of each part of a concern a telling names: the telling that claims it (`Rank`, then the most mentions,
/// then the earlier telling)
fn seeds_of(input: &StepsInput<'_>, mentions: &[Mention], tellings: &[Telling]) -> HashMap<Place, usize> {
  let mut of_unit: HashMap<&str, Vec<Place>> = HashMap::new();
  let mut of_path: HashMap<&str, Vec<Place>> = HashMap::new();
  for (concern, of) in input.concerns.iter().enumerate() {
    for (place, part) in of.parts.iter().enumerate() {
      of_unit.entry(part.unit.as_str()).or_default().push((concern, place));
      of_path.entry(part.path.as_str()).or_default().push((concern, place));
    }
  }
  let telling_of_block: HashMap<usize, usize> = tellings
    .iter()
    .enumerate()
    .flat_map(|(place, telling)| telling.blocks.clone().map(move |block| (block, place)))
    .collect();
  // (place, telling) -> its mentions
  let mut claims: BTreeMap<Place, BTreeMap<usize, u32>> = BTreeMap::new();
  for mention in mentions {
    let Some(&telling) = telling_of_block.get(&(mention.block as usize)) else { continue };
    let places = match &mention.target {
      Some(MentionTarget::Unit { unit, .. }) => of_unit.get(unit.as_str()),
      Some(MentionTarget::File { path, .. }) => of_path.get(path.as_str()),
      None => None,
    };
    for &place in places.into_iter().flatten() {
      *claims.entry(place).or_default().entry(telling).or_default() += 1;
    }
  }
  claims
    .into_iter()
    .filter_map(|(place, by)| {
      by.into_iter()
        .min_by(|(a, count_a), (b, count_b)| tellings[*a].rank.cmp(&tellings[*b].rank).then(count_b.cmp(count_a)).then(a.cmp(b)))
        .map(|(telling, _)| (place, telling))
    })
    .collect()
}

/// The telling each part of a concern goes to (`MessageSteps`); none for a part that stays its concern's
fn assign(input: &StepsInput<'_>, tellings: &[Telling], seeds: &HashMap<Place, usize>, tests: Option<usize>) -> HashMap<Place, usize> {
  let unit_test: HashSet<&str> = input.units.iter().filter(|unit| unit.test).map(|unit| unit.id.as_str()).collect();
  let is_test = |(concern, part): Place| {
    let part = &input.concerns[concern].parts[part];
    test_subject(&part.path).is_some() || unit_test.contains(part.unit.as_str())
  };
  let mut owner: HashMap<Place, usize> = HashMap::new();
  for (concern, of) in input.concerns.iter().enumerate() {
    let places: Vec<Place> = (0..of.parts.len()).map(|part| (concern, part)).collect();
    if of.name == ConcernName::Rest {
      // no evidence joined these: each goes alone
      for &place in &places {
        if let Some(&telling) = seeds.get(&place) {
          owner.insert(place, telling);
        } else if let Some(tests) = tests.filter(|_| is_test(place)) {
          owner.insert(place, tests);
        }
      }
      continue;
    }
    let seeded: BTreeSet<usize> = places.iter().filter_map(|place| seeds.get(place).copied()).collect();
    match seeded.len() {
      0 => {
        if let Some(tests) = tests.filter(|_| places.iter().all(|&place| is_test(place))) {
          for &place in &places {
            owner.insert(place, tests);
          }
        }
      }
      1 => {
        let telling = *seeded.first().expect("one seed");
        for &place in &places {
          owner.insert(place, telling);
        }
      }
      _ => {
        for (place, telling) in cut(of, concern, seeds, tellings) {
          owner.insert(place, telling);
        }
      }
    }
  }
  owner
}

/// The parts of a concern two tellings seed, each with the telling of the nearest seed over the evidence of the concern
/// (`Concern::links`, by their cost): of equal distance the seed of the better rank, then the earlier telling; a part
/// no evidence reaches goes to the telling that seeds most of the concern
fn cut(of: &Concern, concern: usize, seeds: &HashMap<Place, usize>, tellings: &[Telling]) -> Vec<(Place, usize)> {
  let count = of.parts.len();
  let mut next: Vec<Vec<(usize, u32)>> = vec![Vec::new(); count];
  for link in &of.links {
    let (a, b) = (link.a as usize, link.b as usize);
    if a < count && b < count {
      next[a].push((b, u32::from(link.cost)));
      next[b].push((a, u32::from(link.cost)));
    }
  }
  // (distance, rank, telling, part): the least first
  let mut owner: Vec<Option<usize>> = vec![None; count];
  let mut queue: BinaryHeap<Reverse<(u32, Rank, usize, usize)>> = (0..count)
    .filter_map(|part| seeds.get(&(concern, part)).map(|&telling| Reverse((0, tellings[telling].rank, telling, part))))
    .collect();
  while let Some(Reverse((distance, rank, telling, part))) = queue.pop() {
    if owner[part].is_some() {
      continue;
    }
    owner[part] = Some(telling);
    for &(other, cost) in &next[part] {
      if owner[other].is_none() {
        queue.push(Reverse((distance + cost, rank, telling, other)));
      }
    }
  }
  let mut held: BTreeMap<usize, usize> = BTreeMap::new();
  for telling in owner.iter().flatten() {
    *held.entry(*telling).or_default() += 1;
  }
  let most = held.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(telling, _)| *telling);
  owner
    .into_iter()
    .enumerate()
    .filter_map(|(part, telling)| telling.or(most).map(|telling| ((concern, part), telling)))
    .collect()
}

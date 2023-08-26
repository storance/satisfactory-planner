use petgraph::stable_graph::{EdgeIndex, StableDiGraph};
use petgraph::Direction;
use petgraph::{dot::Dot, graph::NodeIndex};

use crate::{
    game::{Item, ItemValuePair, Recipe},
    utils::round_f64,
};
use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Production<'a> {
    pub recipe: &'a Recipe,
    pub machine_count: f64,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum NodeValue<'a> {
    Input(ItemValuePair),
    Output(ItemValuePair),
    ByProduct(ItemValuePair),
    Production(Production<'a>),
}

pub type GraphType<'a> = StableDiGraph<NodeValue<'a>, NodeEdge>;
pub type ScoredGraphType<'a> = StableDiGraph<NodeValue<'a>, ScoredNodeEdge>;

#[allow(dead_code)]
impl<'a> NodeValue<'a> {
    pub fn new_input(input: ItemValuePair) -> Self {
        NodeValue::Input(input)
    }

    pub fn new_output(output: ItemValuePair) -> Self {
        NodeValue::Output(output)
    }

    pub fn new_by_product(output: ItemValuePair) -> Self {
        NodeValue::ByProduct(output)
    }

    pub fn new_production(recipe: &'a Recipe, machine_count: f64) -> Self {
        NodeValue::Production(Production {
            recipe,
            machine_count,
        })
    }

    pub fn is_input(&self) -> bool {
        matches!(self, NodeValue::Input(..))
    }

    pub fn is_output(&self) -> bool {
        matches!(self, NodeValue::Output(..))
    }

    pub fn is_by_product(&self) -> bool {
        matches!(self, NodeValue::ByProduct(..))
    }

    pub fn is_production(&self) -> bool {
        matches!(self, NodeValue::Production(..))
    }

    pub fn as_input(&self) -> &ItemValuePair {
        match self {
            NodeValue::Input(input) => input,
            _ => panic!("NodeValue is not Input"),
        }
    }

    pub fn as_input_mut(&mut self) -> &mut ItemValuePair {
        match self {
            NodeValue::Input(input) => input,
            _ => panic!("NodeValue is not Input"),
        }
    }

    pub fn as_output(&self) -> &ItemValuePair {
        match self {
            NodeValue::Output(output) => output,
            _ => panic!("NodeValue is not Output"),
        }
    }

    pub fn as_output_mut(&mut self) -> &mut ItemValuePair {
        match self {
            NodeValue::Output(output) => output,
            _ => panic!("NodeValue is not Output"),
        }
    }

    pub fn as_by_product(&self) -> &ItemValuePair {
        match self {
            NodeValue::ByProduct(output) => output,
            _ => panic!("NodeValue is not ByProduct"),
        }
    }

    pub fn as_by_product_mut(&mut self) -> &mut ItemValuePair {
        match self {
            NodeValue::ByProduct(output) => output,
            _ => panic!("NodeValue is not ByProduct"),
        }
    }

    pub fn as_production(&self) -> &Production<'a> {
        match self {
            NodeValue::Production(production) => production,
            _ => panic!("NodeValue is not Production"),
        }
    }

    pub fn as_production_mut(&mut self) -> &mut Production<'a> {
        match self {
            NodeValue::Production(production) => production,
            _ => panic!("NodeValue is not Production"),
        }
    }
}

impl<'a> fmt::Display for NodeValue<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeValue::Input(item_value) => {
                write!(
                    f,
                    "{}\n{} / min",
                    item_value.item,
                    round_f64(item_value.value, 3)
                )
            }
            NodeValue::Production(production) => {
                write!(
                    f,
                    "{}\n{}x {}",
                    production.recipe.name,
                    round_f64(production.machine_count, 3),
                    production.recipe.machine
                )
            }
            NodeValue::ByProduct(item_value, ..) => {
                write!(
                    f,
                    "{}\n{} / min",
                    item_value.item,
                    round_f64(item_value.value, 3)
                )
            }
            NodeValue::Output(item_value, ..) => {
                write!(
                    f,
                    "{}\n{} / min",
                    item_value.item,
                    round_f64(item_value.value, 3)
                )
            }
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct NodeEdge {
    pub value: ItemValuePair,
    pub order: u32,
}

impl NodeEdge {
    pub fn new(value: ItemValuePair, order: u32) -> Self {
        Self { value, order }
    }

    #[inline]
    pub fn item(&self) -> Item {
        self.value.item
    }

    #[inline]
    pub fn value(&self) -> f64 {
        self.value.value
    }
}

impl fmt::Display for NodeEdge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}\n{} / min",
            self.value.item,
            round_f64(self.value.value, 3)
        )
    }
}

#[derive(Debug, Clone)]
pub struct PathChain(Vec<u32>);

static ID_GENERATOR: AtomicU32 = AtomicU32::new(0);

impl PathChain {
    pub fn new() -> Self {
        let id = ID_GENERATOR.fetch_add(1, Ordering::Relaxed);
        Self(vec![id])
    }

    pub fn empty() -> Self {
        Self(Vec::new())
    }

    pub fn next(&self) -> Self {
        let mut chain = self.0.clone();
        let id = ID_GENERATOR.fetch_add(1, Ordering::Relaxed);
        chain.push(id);

        Self(chain)
    }

    pub fn is_subset_of(&self, other: &Self) -> bool {
        other.0.starts_with(self.0.as_slice())
    }

    pub fn id(&self) -> u32 {
        self.0.last().copied().unwrap()
    }
}

impl fmt::Display for PathChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}]",
            self.0
                .iter()
                .map(|i| format!("{}", i))
                .collect::<Vec<String>>()
                .join(",")
        )
    }
}

#[derive(Debug, Clone)]
pub struct ScoredNodeEdge {
    pub value: ItemValuePair,
    pub score: f64,
    pub chain: PathChain,
}

impl ScoredNodeEdge {
    pub fn new(value: ItemValuePair, chain: PathChain) -> Self {
        Self {
            value,
            score: f64::INFINITY,
            chain,
        }
    }

    #[inline]
    pub fn item(&self) -> Item {
        self.value.item
    }
}

impl fmt::Display for ScoredNodeEdge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}\n{} / min\nScore: {}\nChain: {}",
            self.value.item,
            round_f64(self.value.value, 3),
            round_f64(self.score, 1),
            self.chain
        )
    }
}

pub fn find_input_node<E>(
    graph: &StableDiGraph<NodeValue<'_>, E>,
    item: Item,
) -> Option<NodeIndex> {
    graph.node_indices().find(|i| match graph[*i] {
        NodeValue::Input(input) => item == input.item,
        _ => false,
    })
}

pub fn find_production_node<E>(
    graph: &StableDiGraph<NodeValue<'_>, E>,
    recipe: &Recipe,
) -> Option<NodeIndex> {
    graph.node_indices().find(|i| match graph[*i] {
        NodeValue::Production(production) => production.recipe == recipe,
        _ => false,
    })
}

pub fn find_output_node<E>(
    graph: &StableDiGraph<NodeValue<'_>, E>,
    item: Item,
) -> Option<NodeIndex> {
    graph.node_indices().find(|i| match graph[*i] {
        NodeValue::Output(output) => item == output.item,
        _ => false,
    })
}

#[allow(dead_code)]
pub fn find_by_product_node<E>(
    graph: &StableDiGraph<NodeValue<'_>, E>,
    item: Item,
) -> Option<NodeIndex> {
    graph.node_indices().find(|i| match graph[*i] {
        NodeValue::ByProduct(output) => item == output.item,
        _ => false,
    })
}

pub fn walk_neighbors_detached<N, E, F>(
    graph: &mut StableDiGraph<N, E>,
    index: NodeIndex,
    dir: Direction,
    mut f: F,
) where
    F: FnMut(&mut StableDiGraph<N, E>, EdgeIndex, NodeIndex),
{
    let mut child_walker = graph.neighbors_directed(index, dir).detach();

    while let Some((edge_index, source_index)) = child_walker.next(graph) {
        f(graph, edge_index, source_index);
    }
}

pub fn print_graph<E: fmt::Display>(graph: &StableDiGraph<NodeValue<'_>, E>) {
    println!(
        "{}",
        format!(
            "{}",
            Dot::with_attr_getters(&graph, &[], &|_, _| String::new(), &|_, n| {
                let color = match n.1 {
                    NodeValue::Input(input) => {
                        if input.item.is_extractable() {
                            "lightslategray"
                        } else {
                            "peru"
                        }
                    }
                    NodeValue::Output(..) => "mediumseagreen",
                    NodeValue::ByProduct(..) => "cornflowerblue",
                    NodeValue::Production(..) => "darkorange",
                };

                format!(
                    "style=\"solid,filled\" shape=\"box\" fontcolor=\"white\" color=\"{}\"",
                    color
                )
            })
        )
        .replace("\\l", "\\n")
    );
}

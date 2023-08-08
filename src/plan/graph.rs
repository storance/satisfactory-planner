use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::game::{Recipe, ResourceValuePair};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
#[repr(transparent)]
pub struct NodeID(usize);

impl NodeID {
    pub fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    pub fn id(&self) -> usize {
        self.0
    }
}

#[derive(Debug)]
pub enum PlanGraphNode<'a> {
    InputNode {
        id: NodeID,
        item: ResourceValuePair<f64>,
    },
    OutputNode {
        id: NodeID,
        item: ResourceValuePair<f64>,
        by_product: bool,
    },
    ProductionNode {
        id: NodeID,
        recipe: &'a Recipe,
        machine_count: f64,
    },
}

pub type NodeType<'a> = Rc<RefCell<PlanGraphNode<'a>>>;
pub type NodeList<'a> = Vec<Rc<RefCell<PlanGraphNode<'a>>>>;

pub struct PlanGraph<'a> {
    pub nodes: NodeList<'a>,
    pub edges_by_parent: HashMap<NodeID, NodeList<'a>>,
    pub edges_by_child: HashMap<NodeID, NodeList<'a>>,
}

impl<'a> PlanGraphNode<'a> {
    pub fn id(&self) -> &NodeID {
        match self {
            PlanGraphNode::InputNode { id, .. } => id,
            PlanGraphNode::OutputNode { id, .. } => id,
            PlanGraphNode::ProductionNode { id, .. } => id,
        }
    }

    pub fn new_input(item: ResourceValuePair<f64>) -> Rc<Self> {
        Rc::new(PlanGraphNode::InputNode {
            id: NodeID::new(),
            item,
        })
    }

    pub fn new_output(item: ResourceValuePair<f64>, by_product: bool) -> Rc<Self> {
        Rc::new(PlanGraphNode::OutputNode {
            id: NodeID::new(),
            item,
            by_product,
        })
    }

    pub fn new_production(recipe: &'a Recipe, machine_count: f64) -> Rc<Self> {
        Rc::new(PlanGraphNode::ProductionNode {
            id: NodeID::new(),
            recipe,
            machine_count,
        })
    }

    pub fn is_input(&self) -> bool {
        match self {
            PlanGraphNode::InputNode { .. } => true,
            _ => false,
        }
    }

    pub fn is_output(&self) -> bool {
        match self {
            PlanGraphNode::OutputNode { .. } => true,
            _ => false,
        }
    }

    pub fn is_production(&self) -> bool {
        match self {
            PlanGraphNode::ProductionNode { .. } => true,
            _ => false,
        }
    }
}

impl<'a> PlanGraph<'a> {
    pub fn add_node(&mut self, node: NodeType<'a>) {
        self.nodes.push(node);
    }

    pub fn add_edge(&mut self, parent: NodeType<'a>, child: NodeType<'a>) {
        let child_id = child.borrow().id().clone();
        let parent_id = parent.borrow().id().clone();

        self.edges_by_child
            .entry(child_id)
            .and_modify(|nodes| nodes.push(Rc::clone(&parent)))
            .or_insert_with(|| vec![Rc::clone(&parent)]);

        self.edges_by_parent
            .entry(parent_id)
            .and_modify(|nodes| nodes.push(Rc::clone(&child)))
            .or_insert_with(|| vec![Rc::clone(&child)]);
    }

    pub fn children(&self, node: &NodeType<'a>) -> Option<&NodeList<'a>> {
        self.edges_by_parent.get(node.borrow().id())
    }

    pub fn parents(&self, node: &NodeType<'a>) -> Option<&NodeList<'a>> {
        self.edges_by_child.get(node.borrow().id())
    }

    pub fn visit_depth_first<F>(&self, mut visitor: F)
    where
        F: FnMut(NodeType<'a>),
    {
        let mut visited: HashSet<NodeID> = HashSet::new();
        self.nodes
            .iter()
            .filter(|node| node.borrow().is_input())
            .for_each(|node| self.visit_depth_first_impl(node, &mut visitor, &mut visited));
    }

    fn visit_depth_first_impl<F>(
        &self,
        node: &NodeType<'a>,
        visitor: &mut F,
        visited: &mut HashSet<NodeID>,
    ) where
        F: FnMut(NodeType<'a>),
    {
        if visited.contains(node.borrow().id()) {
            return;
        }

        visitor(Rc::clone(&node));
        visited.insert(node.borrow().id().clone());

        if let Some(children) = self.children(&node) {
            children
                .iter()
                .for_each(|child| self.visit_depth_first_impl(child, visitor, visited));
        }
    }

    pub fn visit_breath_first<F>(&self, mut visitor: F)
    where
        F: FnMut(NodeType<'a>),
    {
        let mut queue: VecDeque<NodeType<'a>> = VecDeque::new();
        let mut visited: HashSet<NodeID> = HashSet::new();
        self.nodes
            .iter()
            .filter(|node| node.borrow().is_input())
            .for_each(|node| queue.push_back(Rc::clone(node)));

        loop {
            if let Some(node) = queue.pop_front() {
                let node_id = node.borrow().id().clone();
                if !visited.contains(&node_id) {
                    visitor(Rc::clone(&node));
                    visited.insert(node_id);

                    if let Some(children) = self.children(&node) {
                        children
                            .iter()
                            .for_each(|child| queue.push_back(Rc::clone(child)));
                    }
                }
            } else {
                break;
            }
        }
    }
}

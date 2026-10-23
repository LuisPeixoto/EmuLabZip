//! A call tree for flame graphs: stacks of frames merged into one tree, with the time each frame
//! spent running itself and the time it spent in everything it called.

use std::collections::HashMap;

use serde::Serialize;

/// Where a frame's code comes from. The flame graph colours by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    /// The app's own code and libraries.
    App,
    /// The platform's managed code and runtime: the Android framework and ART, UIKit and friends.
    Framework,
    /// System native libraries.
    Native,
    /// The kernel.
    Kernel,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub name: String,
    pub origin: Origin,
    /// Total weight of the samples that passed through this frame, children included.
    pub value: u64,
    /// The part of `value` where this frame was the innermost one.
    pub own: u64,
    /// The number of samples, whatever their weight.
    pub samples: u64,
    /// Largest first.
    pub children: Vec<Node>,
}

impl Node {
    /// Drops the children that account for less than `min_fraction` of `total`, at every level.
    /// Their weight stays in their parents, so totals keep adding up; they are just not drawn.
    pub fn prune(&mut self, total: u64, min_fraction: f64) {
        let floor = (total as f64 * min_fraction).ceil() as u64;
        self.children.retain(|c| c.value >= floor);
        for c in &mut self.children {
            c.prune(total, min_fraction);
        }
    }

    /// The number of nodes below and including this one.
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(Node::count).sum::<usize>()
    }

    fn walk<'a>(&'a self, path: &mut Vec<&'a str>, visit: &mut impl FnMut(&[&'a str], &'a Node)) {
        path.push(&self.name);
        visit(path, self);
        for c in &self.children {
            c.walk(path, visit);
        }
        path.pop();
    }
}

struct Building {
    name: u32,
    origin: Origin,
    value: u64,
    own: u64,
    samples: u64,
    children: HashMap<(u32, Origin), u32>,
}

/// Builds a [`Node`] tree from stacks, one sample at a time.
pub struct FlameBuilder {
    names: Vec<String>,
    ids: HashMap<String, u32>,
    nodes: Vec<Building>,
}

impl FlameBuilder {
    pub fn new(root: &str) -> Self {
        let mut b = FlameBuilder { names: Vec::new(), ids: HashMap::new(), nodes: Vec::new() };
        let name = b.intern(root);
        b.nodes.push(Building {
            name,
            origin: Origin::Framework,
            value: 0,
            own: 0,
            samples: 0,
            children: HashMap::new(),
        });
        b
    }

    fn intern(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.ids.get(name) {
            return id;
        }
        let id = self.names.len() as u32;
        self.names.push(name.to_string());
        self.ids.insert(name.to_string(), id);
        id
    }

    /// Adds one sample. `frames` run from the outermost call to the innermost one.
    pub fn add<'a>(&mut self, frames: impl IntoIterator<Item = (&'a str, Origin)>, weight: u64) {
        let mut at = 0usize;
        self.nodes[0].value += weight;
        self.nodes[0].samples += 1;
        for (name, origin) in frames {
            let name = self.intern(name);
            let next = match self.nodes[at].children.get(&(name, origin)) {
                Some(&i) => i as usize,
                None => {
                    let i = self.nodes.len();
                    self.nodes.push(Building { name, origin, value: 0, own: 0, samples: 0, children: HashMap::new() });
                    self.nodes[at].children.insert((name, origin), i as u32);
                    i
                }
            };
            self.nodes[next].value += weight;
            self.nodes[next].samples += 1;
            at = next;
        }
        self.nodes[at].own += weight;
    }

    pub fn total(&self) -> u64 {
        self.nodes[0].value
    }

    pub fn build(self) -> Node {
        fn convert(b: &FlameBuilder, i: usize) -> Node {
            let n = &b.nodes[i];
            let mut children: Vec<Node> = n.children.values().map(|&c| convert(b, c as usize)).collect();
            // Largest first, with the name as a tie-break so the output does not depend on hashing.
            children.sort_by(|a, c| c.value.cmp(&a.value).then_with(|| a.name.cmp(&c.name)));
            Node {
                name: b.names[n.name as usize].clone(),
                origin: n.origin,
                value: n.value,
                own: n.own,
                samples: n.samples,
                children,
            }
        }
        convert(&self, 0)
    }
}

/// A function and the time it spent running by itself, summed over every place it was called.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hotspot {
    pub name: String,
    pub origin: Origin,
    pub own: u64,
}

/// The functions with the most time of their own, largest first.
pub fn hotspots(root: &Node, limit: usize) -> Vec<Hotspot> {
    let mut by_name: HashMap<(&str, Origin), u64> = HashMap::new();
    root.walk(&mut Vec::new(), &mut |_, n| {
        if n.own > 0 {
            *by_name.entry((n.name.as_str(), n.origin)).or_default() += n.own;
        }
    });
    let mut all: Vec<Hotspot> =
        by_name.into_iter().map(|((name, origin), own)| Hotspot { name: name.to_string(), origin, own }).collect();
    all.sort_by(|a, b| b.own.cmp(&a.own).then_with(|| a.name.cmp(&b.name)));
    all.truncate(limit);
    all
}

/// The tree as folded stacks, one line per stack (`a;b;c 42`). It is the input of Brendan Gregg's
/// `flamegraph.pl` and of tools such as speedscope.
pub fn folded(root: &Node) -> String {
    let mut lines = Vec::new();
    root.walk(&mut Vec::new(), &mut |path, n| {
        if n.own > 0 {
            let stack: Vec<String> = path.iter().map(|p| p.replace(';', ":")).collect();
            lines.push(format!("{} {}", stack.join(";"), n.own));
        }
    });
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tree() -> Node {
        let mut b = FlameBuilder::new("all");
        b.add([("main", Origin::App), ("parse", Origin::App), ("memcpy", Origin::Native)], 6);
        b.add([("main", Origin::App), ("parse", Origin::App)], 2);
        b.add([("main", Origin::App), ("draw", Origin::Framework)], 4);
        b.add([("main", Origin::App), ("parse", Origin::App), ("memcpy", Origin::Native)], 3);
        b.add([("idle", Origin::Kernel)], 1);
        b.build()
    }

    #[test]
    fn merges_stacks_and_adds_up_weights() {
        let t = sample_tree();
        assert_eq!((t.value, t.samples), (16, 5));
        let main = &t.children[0];
        assert_eq!((main.name.as_str(), main.value, main.own), ("main", 15, 0));
        let parse = &main.children[0];
        assert_eq!((parse.name.as_str(), parse.value, parse.own), ("parse", 11, 2));
        let memcpy = &parse.children[0];
        assert_eq!((memcpy.value, memcpy.own, memcpy.samples), (9, 9, 2));
        // A parent's weight is its own plus its children's.
        fn check(n: &Node) {
            assert_eq!(n.value, n.own + n.children.iter().map(|c| c.value).sum::<u64>(), "{}", n.name);
            n.children.iter().for_each(check);
        }
        check(&t);
    }

    #[test]
    fn orders_children_by_weight() {
        let t = sample_tree();
        let names: Vec<_> = t.children[0].children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["parse", "draw"]);
    }

    #[test]
    fn the_same_name_with_another_origin_is_another_frame() {
        let mut b = FlameBuilder::new("all");
        b.add([("run", Origin::App)], 1);
        b.add([("run", Origin::Native)], 1);
        assert_eq!(b.build().children.len(), 2);
    }

    #[test]
    fn finds_the_hottest_functions_across_call_sites() {
        let mut b = FlameBuilder::new("all");
        b.add([("a", Origin::App), ("memcpy", Origin::Native)], 3);
        b.add([("b", Origin::App), ("memcpy", Origin::Native)], 4);
        b.add([("b", Origin::App)], 5);
        let hot = hotspots(&b.build(), 2);
        assert_eq!(hot.iter().map(|h| (h.name.as_str(), h.own)).collect::<Vec<_>>(), [("memcpy", 7), ("b", 5)]);
    }

    #[test]
    fn writes_folded_stacks() {
        let out = folded(&sample_tree());
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines.contains(&"all;main;parse;memcpy 9"));
        assert!(lines.contains(&"all;main;parse 2"));
        assert!(lines.contains(&"all;idle 1"));
        // The weights of the lines add up to the total.
        let sum: u64 = lines.iter().map(|l| l.rsplit(' ').next().unwrap().parse::<u64>().unwrap()).sum();
        assert_eq!(sum, 16);
    }

    #[test]
    fn pruning_hides_small_frames_but_keeps_totals() {
        let mut t = sample_tree();
        let before = t.count();
        t.prune(t.value, 0.1);
        assert!(t.count() < before);
        assert_eq!(t.value, 16);
        assert!(t.children.iter().all(|c| c.name != "idle"));
    }
}

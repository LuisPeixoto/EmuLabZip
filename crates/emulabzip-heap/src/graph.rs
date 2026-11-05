//! The object graph: shortest paths from the roots, the dominator tree and retained sizes.

use crate::hprof::Heap;

/// The result of walking the graph from the garbage collector's roots.
pub struct Reach {
    /// For each object, the object that first reached it (`u32::MAX` for roots and unreachable ones).
    pub parent: Vec<u32>,
    /// Whether the object can be reached from a root at all.
    pub reachable: Vec<bool>,
    /// The immediate dominator of each object (`u32::MAX` for those that only the roots dominate).
    pub idom: Vec<u32>,
    /// Bytes that would be freed with the object: its own size and everything only it keeps alive.
    pub retained: Vec<u64>,
}

const NONE: u32 = u32::MAX;

pub fn analyze(heap: &Heap) -> Reach {
    let n = heap.objects.len();
    let root = n as u32; // a virtual object that holds every root

    let succ = |v: u32| -> &[u32] { if v == root { &[] } else { heap.successors(v) } };
    let mut root_list: Vec<u32> = heap.roots.iter().map(|r| r.0).collect();
    root_list.sort_unstable();
    root_list.dedup();

    // Breadth-first: the shortest path from a root to each object.
    let mut parent = vec![NONE; n];
    let mut reachable = vec![false; n];
    let mut queue: Vec<u32> = Vec::with_capacity(n);
    for &r in &root_list {
        reachable[r as usize] = true;
        queue.push(r);
    }
    let mut head = 0;
    while head < queue.len() {
        let v = queue[head];
        head += 1;
        for &w in succ(v) {
            if !reachable[w as usize] {
                reachable[w as usize] = true;
                parent[w as usize] = v;
                queue.push(w);
            }
        }
    }

    // Depth-first order for the dominator algorithm (iterative: a chain can be a million objects long).
    let mut post = vec![NONE; n + 1]; // postorder number of each node
    let mut order: Vec<u32> = Vec::with_capacity(queue.len() + 1); // nodes by postorder
    let mut seen = vec![false; n + 1];
    let mut stack: Vec<(u32, usize)> = vec![(root, 0)];
    seen[root as usize] = true;
    while let Some(&(v, i)) = stack.last() {
        let next = if v == root { root_list.get(i).copied() } else { succ(v).get(i).copied() };
        match next {
            Some(w) => {
                if let Some(top) = stack.last_mut() {
                    top.1 += 1;
                }
                if !seen[w as usize] {
                    seen[w as usize] = true;
                    stack.push((w, 0));
                }
            }
            None => {
                post[v as usize] = order.len() as u32;
                order.push(v);
                stack.pop();
            }
        }
    }

    // Predecessors of the reachable nodes, as a compact list.
    let mut pred_count = vec![0u32; n + 2];
    for &v in &order {
        let targets: &[u32] = if v == root { &root_list } else { succ(v) };
        for &w in targets {
            pred_count[w as usize + 1] += 1;
        }
    }
    for i in 1..pred_count.len() {
        pred_count[i] += pred_count[i - 1];
    }
    let mut fill = pred_count.clone();
    let mut preds = vec![0u32; *pred_count.last().unwrap_or(&0) as usize];
    for &v in &order {
        let targets: &[u32] = if v == root { &root_list } else { succ(v) };
        for &w in targets {
            preds[fill[w as usize] as usize] = v;
            fill[w as usize] += 1;
        }
    }

    // Cooper, Harvey and Kennedy: iterate until the immediate dominators stop changing.
    let mut dom = vec![NONE; n + 1];
    dom[root as usize] = root;
    let intersect = |dom: &[u32], mut a: u32, mut b: u32| -> u32 {
        while a != b {
            while post[a as usize] < post[b as usize] {
                a = dom[a as usize];
            }
            while post[b as usize] < post[a as usize] {
                b = dom[b as usize];
            }
        }
        a
    };
    let mut changed = true;
    while changed {
        changed = false;
        for &v in order.iter().rev() {
            if v == root {
                continue;
            }
            let mut new = NONE;
            for &p in &preds[pred_count[v as usize] as usize..pred_count[v as usize + 1] as usize] {
                if dom[p as usize] == NONE {
                    continue;
                }
                new = if new == NONE { p } else { intersect(&dom, p, new) };
            }
            if new != dom[v as usize] {
                dom[v as usize] = new;
                changed = true;
            }
        }
    }

    // Retained sizes: children are finished before their dominators in postorder.
    let mut retained: Vec<u64> = (0..=n).map(|i| if i < n { u64::from(heap.objects[i].shallow) } else { 0 }).collect();
    for &v in &order {
        if v == root {
            continue;
        }
        let d = dom[v as usize];
        if d != NONE && d != v {
            let add = retained[v as usize];
            retained[d as usize] += add;
        }
    }
    retained.truncate(n);

    let idom: Vec<u32> = (0..n).map(|i| if dom[i] == root || dom[i] == NONE { NONE } else { dom[i] }).collect();
    Reach { parent, reachable, idom, retained }
}

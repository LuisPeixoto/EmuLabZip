//! Looking for objects that should be gone, and the chain of references that keeps them alive.

use std::collections::HashMap;

use serde::Serialize;

use crate::graph::{self, Reach};
use crate::hprof::{Error, Heap, Kind, RootKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    /// The object says it is finished with (`mDestroyed`, no `mFragmentManager`) and is still alive.
    Certain,
    /// An `Activity` that is finishing but not destroyed yet: a leak if it stays that way.
    Possible,
}

/// One link of a path: the object, and the field of it that holds the next one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub class: String,
    /// The field that points to the next step; empty for the first step when the root is the class itself,
    /// and for the last step.
    pub field: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leak {
    /// `Activity` or `Fragment`.
    pub kind: String,
    pub class: String,
    /// The address of the object in the dump, as the dump writes it.
    pub id: String,
    pub reason: String,
    pub confidence: Confidence,
    pub retained_bytes: u64,
    /// What holds the chain up: a system class, a thread, a JNI reference...
    pub root: String,
    /// From the first object after the root to the leaked one.
    pub path: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassStat {
    pub name: String,
    pub count: u64,
    pub shallow_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dominator {
    pub class: String,
    pub id: String,
    pub shallow_bytes: u64,
    pub retained_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub objects: u64,
    pub reachable_objects: u64,
    pub shallow_bytes: u64,
    /// The dump was cut short, and what was read is analyzed.
    pub truncated: bool,
    /// Heap names found in the dump (`app`, `zygote`, `image`), empty for a plain Java dump.
    pub heaps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    pub summary: Summary,
    pub leaks: Vec<Leak>,
    pub dominators: Vec<Dominator>,
    pub classes: Vec<ClassStat>,
}

#[derive(Debug, Clone)]
pub struct Options {
    /// How many dominators and classes to keep.
    pub top: usize,
    /// Look for leaks in every heap, not only the app's.
    pub all_heaps: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { top: 25, all_heaps: false }
    }
}

const ACTIVITY: &str = "android.app.Activity";
const FRAGMENTS: [&str; 3] =
    ["androidx.fragment.app.Fragment", "android.app.Fragment", "android.support.v4.app.Fragment"];

/// Reads a dump and analyzes it.
pub fn analyze(bytes: &[u8], options: &Options) -> Result<Analysis, Error> {
    let heap = Heap::read(bytes)?;
    let reach = graph::analyze(&heap);
    Ok(report(&heap, &reach, options))
}

fn hex(id: u64) -> String {
    format!("0x{id:x}")
}

fn bool_field(heap: &Heap, obj: u32, name: &str) -> Option<bool> {
    heap.fields_of(obj).into_iter().find(|(_, f, _)| f.name == name && f.ty == 4).map(|(_, _, v)| v & 0xff != 0)
}

fn ref_field_is_null(heap: &Heap, obj: u32, name: &str) -> Option<bool> {
    heap.fields_of(obj).into_iter().find(|(_, f, _)| f.name == name && f.ty == 2).map(|(_, _, v)| v == 0)
}

/// The field of `from` whose value is the object `to`.
fn field_pointing_to(heap: &Heap, from: u32, to: u32) -> Option<String> {
    let target = heap.objects[to as usize].id;
    let o = &heap.objects[from as usize];
    match o.kind {
        Kind::Instance => {
            heap.fields_of(from).into_iter().find(|(_, f, v)| f.ty == 2 && *v == target).map(|(_, f, _)| f.name.clone())
        }
        Kind::Class => heap.classes[o.class as usize].static_refs.iter().find(|s| s.1 == target).map(|s| s.0.clone()),
        Kind::ObjectArray => Some("[]".to_string()),
        Kind::PrimitiveArray => None,
    }
}

fn path_to(heap: &Heap, reach: &Reach, leaked: u32, roots: &HashMap<u32, RootKind>) -> (String, Vec<Step>) {
    let mut chain = vec![leaked];
    let mut cur = leaked;
    while reach.parent[cur as usize] != u32::MAX && chain.len() < 512 {
        cur = reach.parent[cur as usize];
        chain.push(cur);
    }
    chain.reverse();
    // A class that is not a root of its own was reached through its class loader. What matters is the static
    // field of the last class on the way, so the path starts there.
    let mut root = roots.get(&chain[0]).map(|k| k.label()).unwrap_or("root").to_string();
    if let Some(i) = chain.iter().rposition(|&o| heap.objects[o as usize].kind == Kind::Class)
        && i > 0
    {
        chain.drain(..i);
        root = "static field".to_string();
    }
    let steps = chain
        .iter()
        .enumerate()
        .map(|(i, &o)| {
            let obj = &heap.objects[o as usize];
            let class = if obj.kind == Kind::Class {
                format!("class {}", heap.classes[obj.class as usize].name)
            } else {
                heap.class_of(o).name.clone()
            };
            let field = chain.get(i + 1).and_then(|&next| field_pointing_to(heap, o, next));
            Step { class, field }
        })
        .collect();
    (root, steps)
}

fn report(heap: &Heap, reach: &Reach, options: &Options) -> Analysis {
    let n = heap.objects.len();
    let roots: HashMap<u32, RootKind> = heap.roots.iter().map(|&(o, k)| (o, k)).collect();
    let app_only = !options.all_heaps && heap.heap_names.values().any(|h| h == "app");
    let in_scope = |o: u32| !app_only || heap.heap_of(o) == Some("app");

    // Leaks.
    let mut leaks = Vec::new();
    for o in 0..n as u32 {
        if heap.objects[o as usize].kind != Kind::Instance || !reach.reachable[o as usize] || !in_scope(o) {
            continue;
        }
        let found = if heap.is_a(o, ACTIVITY) {
            match (bool_field(heap, o, "mDestroyed"), bool_field(heap, o, "mFinished")) {
                (Some(true), _) => {
                    Some(("Activity", "it was destroyed, but something still holds it", Confidence::Certain))
                }
                (_, Some(true)) => {
                    Some(("Activity", "it is finishing, and something still holds it", Confidence::Possible))
                }
                _ => None,
            }
        } else if FRAGMENTS.iter().any(|f| heap.is_a(o, f)) {
            match ref_field_is_null(heap, o, "mFragmentManager") {
                Some(true) => Some((
                    "Fragment",
                    "it was removed (it has no fragment manager), but something still holds it",
                    Confidence::Certain,
                )),
                _ => None,
            }
        } else {
            None
        };
        if let Some((kind, reason, confidence)) = found {
            let (root, path) = path_to(heap, reach, o, &roots);
            leaks.push(Leak {
                kind: kind.into(),
                class: heap.class_of(o).name.clone(),
                id: hex(heap.objects[o as usize].id),
                reason: reason.into(),
                confidence,
                retained_bytes: reach.retained[o as usize],
                root,
                path,
            });
        }
    }
    leaks.sort_by(|a, b| {
        (a.confidence as u8)
            .cmp(&(b.confidence as u8))
            .then(b.retained_bytes.cmp(&a.retained_bytes))
            .then(a.id.cmp(&b.id))
    });

    // Biggest dominators: the instances that keep the most alive.
    let mut by_retained: Vec<u32> = (0..n as u32)
        .filter(|&o| reach.reachable[o as usize] && heap.objects[o as usize].kind != Kind::Class && in_scope(o))
        .collect();
    by_retained.sort_by(|&a, &b| reach.retained[b as usize].cmp(&reach.retained[a as usize]).then(a.cmp(&b)));
    let dominators = by_retained
        .iter()
        .take(options.top)
        .map(|&o| Dominator {
            class: heap.class_of(o).name.clone(),
            id: hex(heap.objects[o as usize].id),
            shallow_bytes: u64::from(heap.objects[o as usize].shallow),
            retained_bytes: reach.retained[o as usize],
        })
        .collect();

    // Classes by shallow size.
    let mut stats: HashMap<u32, (u64, u64)> = HashMap::new();
    for o in 0..n as u32 {
        let ob = &heap.objects[o as usize];
        if ob.kind == Kind::Class || !reach.reachable[o as usize] || !in_scope(o) {
            continue;
        }
        let e = stats.entry(ob.class).or_default();
        e.0 += 1;
        e.1 += u64::from(ob.shallow);
    }
    let mut classes: Vec<ClassStat> = stats
        .into_iter()
        .map(|(c, (count, shallow))| ClassStat {
            name: heap.classes[c as usize].name.clone(),
            count,
            shallow_bytes: shallow,
        })
        .collect();
    classes.sort_by(|a, b| b.shallow_bytes.cmp(&a.shallow_bytes).then(a.name.cmp(&b.name)));
    classes.truncate(options.top);

    let mut heaps: Vec<String> = heap.heap_names.values().filter(|v| !v.is_empty()).cloned().collect();
    heaps.sort();
    heaps.dedup();
    Analysis {
        summary: Summary {
            objects: n as u64,
            reachable_objects: reach.reachable.iter().filter(|&&r| r).count() as u64,
            shallow_bytes: heap.objects.iter().map(|o| u64::from(o.shallow)).sum(),
            truncated: heap.truncated,
            heaps,
        },
        leaks,
        dominators,
        classes,
    }
}

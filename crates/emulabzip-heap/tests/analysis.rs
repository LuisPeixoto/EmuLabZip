use emulabzip_heap::graph;
use emulabzip_heap::testing::{Builder, Value};
use emulabzip_heap::{Confidence, Error, Heap, Options, analyze};

const OBJECT: u64 = 1;
const ACTIVITY: u64 = 2;
const MAIN: u64 = 3;
const HOLDER: u64 = 4;

/// `Holder.sInstance` keeps `MainActivity` alive; the activity's flags are given.
fn activity_dump(destroyed: bool, finished: bool) -> Vec<u8> {
    let mut b = Builder::new();
    b.class(OBJECT, "java.lang.Object", 0, &[], &[], true)
        .class(ACTIVITY, "android.app.Activity", OBJECT, &[("mDestroyed", 4), ("mFinished", 4)], &[], true)
        .class(MAIN, "com.example.MainActivity", ACTIVITY, &[], &[], true)
        .class(HOLDER, "com.example.Holder", OBJECT, &[], &[("sInstance", 100)], true)
        .instance(100, MAIN, &[Value::Bool(destroyed), Value::Bool(finished)]);
    b.finish()
}

#[test]
fn a_destroyed_activity_held_by_a_static_field_is_a_leak_with_its_path() {
    let a = analyze(&activity_dump(true, true), &Options::default()).unwrap();
    assert_eq!(a.leaks.len(), 1, "{:?}", a.leaks);
    let leak = &a.leaks[0];
    assert_eq!(
        (leak.kind.as_str(), leak.class.as_str(), leak.confidence),
        ("Activity", "com.example.MainActivity", Confidence::Certain)
    );
    assert_eq!(leak.root, "system class");
    let path: Vec<(&str, Option<&str>)> = leak.path.iter().map(|s| (s.class.as_str(), s.field.as_deref())).collect();
    assert_eq!(path, [("class com.example.Holder", Some("sInstance")), ("com.example.MainActivity", None)]);
    assert!(leak.retained_bytes >= 8, "{}", leak.retained_bytes);
}

#[test]
fn a_live_or_finishing_activity_is_not_called_a_certain_leak() {
    assert!(analyze(&activity_dump(false, false), &Options::default()).unwrap().leaks.is_empty());
    let a = analyze(&activity_dump(false, true), &Options::default()).unwrap();
    assert_eq!(a.leaks.len(), 1);
    assert_eq!(a.leaks[0].confidence, Confidence::Possible);
}

#[test]
fn a_weak_reference_does_not_keep_an_activity_alive() {
    let (reference, weak) = (10, 11);
    let mut b = Builder::new();
    b.class(OBJECT, "java.lang.Object", 0, &[], &[], true)
        .class(ACTIVITY, "android.app.Activity", OBJECT, &[("mDestroyed", 4), ("mFinished", 4)], &[], true)
        .class(MAIN, "com.example.MainActivity", ACTIVITY, &[], &[], true)
        .class(reference, "java.lang.ref.Reference", OBJECT, &[("referent", 2)], &[], true)
        .class(weak, "java.lang.ref.WeakReference", reference, &[], &[], true)
        .instance(100, MAIN, &[Value::Bool(true), Value::Bool(true)])
        .instance(101, weak, &[Value::Ref(100)])
        .root(0x03, 101);
    let a = analyze(&b.finish(), &Options::default()).unwrap();
    assert!(a.leaks.is_empty(), "only a weak reference points to it: {:?}", a.leaks);
}

#[test]
fn a_removed_fragment_that_is_still_held_is_a_leak() {
    let (fragment, mine) = (20, 21);
    let mut b = Builder::new();
    b.class(OBJECT, "java.lang.Object", 0, &[], &[], true)
        .class(fragment, "androidx.fragment.app.Fragment", OBJECT, &[("mFragmentManager", 2)], &[], true)
        .class(mine, "com.example.ProfileFragment", fragment, &[], &[], true)
        .class(HOLDER, "com.example.Cache", OBJECT, &[], &[("last", 100)], true)
        .instance(100, mine, &[Value::Ref(0)]);
    let a = analyze(&b.finish(), &Options::default()).unwrap();
    assert_eq!(a.leaks.len(), 1);
    assert_eq!((a.leaks[0].kind.as_str(), a.leaks[0].class.as_str()), ("Fragment", "com.example.ProfileFragment"));
}

#[test]
fn retained_sizes_follow_the_dominator_tree() {
    // A holds B and C, which both hold D. Only A dominates D.
    let node = 30;
    let mut b = Builder::new();
    b.class(OBJECT, "java.lang.Object", 0, &[], &[], true)
        .class(node, "com.example.Node", OBJECT, &[("left", 2), ("right", 2)], &[], true)
        .instance(1000, node, &[Value::Ref(1001), Value::Ref(1002)])
        .instance(1001, node, &[Value::Ref(1003), Value::Ref(0)])
        .instance(1002, node, &[Value::Ref(1003), Value::Ref(0)])
        .instance(1003, node, &[Value::Ref(0), Value::Ref(0)])
        .root(0x03, 1000);
    let bytes = b.finish();
    let heap = Heap::read(&bytes).unwrap();
    let reach = graph::analyze(&heap);
    let retained = |id: u64| reach.retained[heap.object_index_of(id).unwrap() as usize];
    let each = 16; // 8 header and two 4-byte fields
    assert_eq!(retained(1003), each);
    assert_eq!(retained(1001), each, "B does not own D");
    assert_eq!(retained(1002), each);
    assert_eq!(retained(1000), 4 * each, "A owns B, C and D");
    let top = analyze(&bytes, &Options::default()).unwrap();
    assert_eq!(top.dominators[0].retained_bytes, 4 * each);
}

#[test]
fn only_the_app_heap_is_searched_by_default() {
    let mut b = Builder::new();
    b.class(OBJECT, "java.lang.Object", 0, &[], &[], true)
        .class(ACTIVITY, "android.app.Activity", OBJECT, &[("mDestroyed", 4), ("mFinished", 4)], &[], true)
        .class(MAIN, "com.example.MainActivity", ACTIVITY, &[], &[], true)
        .class(HOLDER, "com.example.Holder", OBJECT, &[], &[("sInstance", 100)], true)
        .heap(1, "zygote")
        .instance(100, MAIN, &[Value::Bool(true), Value::Bool(true)])
        .heap(2, "app");
    let bytes = b.finish();
    assert!(analyze(&bytes, &Options::default()).unwrap().leaks.is_empty(), "the activity is in the zygote heap");
    // ... but the heap names are the only thing that tells: with none of them named `app`, everything is searched.
    let all = analyze(&bytes, &Options { all_heaps: true, ..Options::default() }).unwrap();
    assert_eq!(all.leaks.len(), 1);
}

#[test]
fn what_is_not_a_heap_dump_is_refused() {
    assert_eq!(analyze(b"", &Options::default()).unwrap_err(), Error::NotHprof);
    assert_eq!(analyze(b"hello, world", &Options::default()).unwrap_err(), Error::NotHprof);
    assert!(matches!(
        analyze(b"JAVA PROFILE 1.0.3\0\0\0\0\x03", &Options::default()),
        Err(Error::Invalid(_) | Error::Truncated)
    ));
}

#[test]
fn a_dump_cut_anywhere_never_panics() {
    let full = activity_dump(true, true);
    for end in 0..full.len() {
        let _ = analyze(&full[..end], &Options::default());
    }
    // And one cut at the end of the last full record still analyzes what it has.
    let whole = analyze(&full, &Options::default()).unwrap();
    assert!(!whole.summary.truncated);
}

#[test]
fn damaged_lengths_do_not_panic_or_allocate_without_limit() {
    let mut bytes = activity_dump(true, true);
    // Flip every byte in turn: the reader may refuse or may understand, but it must not crash.
    for i in 0..bytes.len() {
        let old = bytes[i];
        bytes[i] = old ^ 0xff;
        let _ = analyze(&bytes, &Options::default());
        bytes[i] = old;
    }
}

#[test]
fn a_very_deep_chain_is_walked_without_recursion() {
    // 200,000 nodes in a line: a recursive walk would overflow the stack.
    let node = 30;
    let mut b = Builder::new();
    b.class(OBJECT, "java.lang.Object", 0, &[], &[], true).class(
        node,
        "com.example.Node",
        OBJECT,
        &[("next", 2)],
        &[],
        true,
    );
    let n = 200_000u64;
    for i in 0..n {
        b.instance(10_000 + i, node, &[Value::Ref(if i + 1 < n { 10_000 + i + 1 } else { 0 })]);
    }
    b.root(0x03, 10_000);
    let bytes = b.finish();
    let started = std::time::Instant::now();
    let a = analyze(&bytes, &Options::default()).unwrap();
    assert_eq!(a.summary.reachable_objects, n + 2, "the nodes and the two classes");
    assert_eq!(a.dominators[0].retained_bytes, n * 12);
    assert!(started.elapsed() < std::time::Duration::from_secs(20), "{:?}", started.elapsed());
}

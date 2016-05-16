extern crate ralloc;

use std::collections::BTreeMap;

#[test]
fn test() {
    let mut map = BTreeMap::new();

    map.insert("Nicolas", "Cage");
    map.insert("is", "God");
    map.insert("according", "to");
    map.insert("ca1ek", ".");

    assert_eq!(map.get("Nicolas"), Some(&"Cage"));
    assert_eq!(map.get("is"), Some(&"God"));
    assert_eq!(map.get("according"), Some(&"to"));
    assert_eq!(map.get("ca1ek"), Some(&"."));
    assert_eq!(map.get("This doesn't exist."), None);
}

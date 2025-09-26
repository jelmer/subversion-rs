//! Sorting utilities for Subversion paths and hash tables
//!
//! This module provides utilities for sorting paths and hash tables in ways
//! that are useful for Subversion operations, such as lexicographic path sorting.

use std::cmp::Ordering;
use std::collections::HashMap;

/// Compare two items as paths for sorting
///
/// This function implements path-aware comparison that follows Subversion's
/// path sorting conventions. Since the SVN C function is not available in
/// the bindings, we implement a path-aware comparison ourselves.
pub fn compare_items_as_paths(a: &str, b: &str) -> Ordering {
    // Implement path-aware comparison similar to SVN's logic
    // This handles cases like:
    // - "dir" should come before "dir/file"
    // - "file1" should come before "file10" (but we'll use lexicographic for simplicity)

    // Split paths into components for comparison
    let a_parts: Vec<&str> = a.split('/').collect();
    let b_parts: Vec<&str> = b.split('/').collect();

    // Compare component by component
    for (a_part, b_part) in a_parts.iter().zip(b_parts.iter()) {
        match a_part.cmp(b_part) {
            Ordering::Equal => continue,
            other => return other,
        }
    }

    // If all common parts are equal, the shorter path comes first
    a_parts.len().cmp(&b_parts.len())
}

/// Sort a vector of paths using Subversion's path comparison
pub fn sort_paths(paths: &mut [String]) {
    paths.sort_by(|a, b| compare_items_as_paths(a, b));
}

/// Sort a HashMap by keys using path comparison and return sorted key-value pairs
pub fn sort_hash_by_paths<V>(hash: &HashMap<String, V>) -> Vec<(&String, &V)> {
    let mut items: Vec<_> = hash.iter().collect();
    items.sort_by(|a, b| compare_items_as_paths(a.0, b.0));
    items
}

/// Sort a HashMap by keys and return a new HashMap (requires Clone for values)
pub fn sort_hash_to_ordered<V: Clone>(hash: &HashMap<String, V>) -> indexmap::IndexMap<String, V> {
    let sorted_items = sort_hash_by_paths(hash);
    let mut ordered = indexmap::IndexMap::new();
    for (key, value) in sorted_items {
        ordered.insert(key.clone(), value.clone());
    }
    ordered
}

/// Compare paths in a way that directories come before files
///
/// This is useful when you want directories to be processed before their contents.
pub fn compare_paths_dirs_first(a: &str, b: &str) -> Ordering {
    let a_is_dir = a.ends_with('/');
    let b_is_dir = b.ends_with('/');

    match (a_is_dir, b_is_dir) {
        (true, false) => Ordering::Less,    // directories come first
        (false, true) => Ordering::Greater, // files come after directories
        _ => compare_items_as_paths(a, b),  // same type, use path comparison
    }
}

/// Sort paths with directories first
pub fn sort_paths_dirs_first(paths: &mut [String]) {
    paths.sort_by(|a, b| compare_paths_dirs_first(a, b));
}

/// Path depth comparison - shallower paths come first
pub fn compare_paths_by_depth(a: &str, b: &str) -> Ordering {
    let a_depth = a.matches('/').count();
    let b_depth = b.matches('/').count();

    match a_depth.cmp(&b_depth) {
        Ordering::Equal => compare_items_as_paths(a, b), // Same depth, use path comparison
        other => other,
    }
}

/// Sort paths by depth (shallower first), then by path
pub fn sort_paths_by_depth(paths: &mut [String]) {
    paths.sort_by(|a, b| compare_paths_by_depth(a, b));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_comparison() {
        // Test basic path comparison
        assert_eq!(compare_items_as_paths("a", "b"), Ordering::Less);
        assert_eq!(compare_items_as_paths("b", "a"), Ordering::Greater);
        assert_eq!(compare_items_as_paths("same", "same"), Ordering::Equal);
    }

    #[test]
    fn test_sort_paths() {
        let mut paths = vec![
            "dir/file2.txt".to_string(),
            "dir/file1.txt".to_string(),
            "another/path.txt".to_string(),
            "dir/subdir/file.txt".to_string(),
        ];

        sort_paths(&mut paths);

        // Should be sorted lexicographically
        assert_eq!(paths[0], "another/path.txt");
        assert_eq!(paths[1], "dir/file1.txt");
        assert_eq!(paths[2], "dir/file2.txt");
        assert_eq!(paths[3], "dir/subdir/file.txt");
    }

    #[test]
    fn test_sort_hash_by_paths() {
        let mut hash = HashMap::new();
        hash.insert("zebra".to_string(), 1);
        hash.insert("apple".to_string(), 2);
        hash.insert("banana".to_string(), 3);

        let sorted = sort_hash_by_paths(&hash);

        // Should be sorted by keys
        assert_eq!(sorted[0].0, "apple");
        assert_eq!(sorted[1].0, "banana");
        assert_eq!(sorted[2].0, "zebra");
    }

    #[test]
    fn test_dirs_first_sorting() {
        let mut paths = vec![
            "dir/file.txt".to_string(),
            "dir/".to_string(),
            "file.txt".to_string(),
            "another_dir/".to_string(),
        ];

        sort_paths_dirs_first(&mut paths);

        // Directories should come before files
        assert!(paths[0].ends_with('/'));
        assert!(paths[1].ends_with('/'));
        assert!(!paths[2].ends_with('/'));
        assert!(!paths[3].ends_with('/'));
    }

    #[test]
    fn test_depth_sorting() {
        let mut paths = vec![
            "deep/path/to/file.txt".to_string(),
            "file.txt".to_string(),
            "dir/file.txt".to_string(),
            "very/deep/path/to/another/file.txt".to_string(),
        ];

        sort_paths_by_depth(&mut paths);

        // Should be sorted by depth (number of slashes)
        assert_eq!(paths[0], "file.txt"); // depth 0
        assert_eq!(paths[1], "dir/file.txt"); // depth 1
        assert_eq!(paths[2], "deep/path/to/file.txt"); // depth 3
        assert_eq!(paths[3], "very/deep/path/to/another/file.txt"); // depth 5
    }

    #[test]
    fn test_sort_hash_to_ordered() {
        let mut hash = HashMap::new();
        hash.insert("c".to_string(), "third".to_string());
        hash.insert("a".to_string(), "first".to_string());
        hash.insert("b".to_string(), "second".to_string());

        let ordered = sort_hash_to_ordered(&hash);
        let keys: Vec<_> = ordered.keys().collect();

        assert_eq!(keys, vec!["a", "b", "c"]);
        assert_eq!(ordered["a"], "first");
        assert_eq!(ordered["b"], "second");
        assert_eq!(ordered["c"], "third");
    }
}

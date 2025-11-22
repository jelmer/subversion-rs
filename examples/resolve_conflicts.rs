use subversion::{
    client::{CheckoutOptions, CommitOptions, Context, UpdateOptions},
    Depth, Revision,
};

/// Example demonstrating the new conflict resolution API
///
/// This example shows how to:
/// 1. Create a repository with conflicts
/// 2. Detect conflicts using conflict_get
/// 3. Examine conflict details
/// 4. Resolve conflicts using various methods
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Conflict Resolution API Example\n");

    // Create a temporary directory for our test
    let temp_dir = tempfile::tempdir()?;
    let repo_path = temp_dir.path().join("repo");
    let wc1_path = temp_dir.path().join("wc1");
    let wc2_path = temp_dir.path().join("wc2");

    // Create a repository
    println!("Creating repository...");
    subversion::repos::Repos::create(&repo_path)?;

    let mut ctx = Context::new()?;
    let url = format!("file://{}", repo_path.display());

    // Checkout first working copy
    println!("Checking out working copy 1...");
    ctx.checkout(
        url.as_str(),
        &wc1_path,
        &CheckoutOptions {
            peg_revision: Revision::Head,
            revision: Revision::Head,
            depth: Depth::Infinity,
            ignore_externals: false,
            allow_unver_obstructions: false,
        },
    )?;

    // Create a file and commit
    println!("Creating initial file...");
    let file1 = wc1_path.join("test.txt");
    std::fs::write(&file1, "Line 1\nLine 2\nLine 3\n")?;

    ctx.add(
        &file1,
        &subversion::client::AddOptions {
            depth: Depth::Empty,
            force: false,
            no_ignore: false,
            no_autoprops: false,
            add_parents: false,
        },
    )?;

    ctx.commit(
        &[wc1_path.to_str().unwrap()],
        &CommitOptions::default(),
        std::collections::HashMap::new(),
        &|_| Ok(()),
    )?;

    // Checkout second working copy
    println!("Checking out working copy 2...");
    ctx.checkout(
        url.as_str(),
        &wc2_path,
        &CheckoutOptions {
            peg_revision: Revision::Head,
            revision: Revision::Head,
            depth: Depth::Infinity,
            ignore_externals: false,
            allow_unver_obstructions: false,
        },
    )?;

    // Modify file in wc1 and commit
    println!("Modifying file in wc1...");
    std::fs::write(&file1, "Line 1\nModified by WC1\nLine 3\n")?;
    ctx.commit(
        &[wc1_path.to_str().unwrap()],
        &CommitOptions::default(),
        std::collections::HashMap::new(),
        &|_| Ok(()),
    )?;

    // Modify same file in wc2 (different change)
    println!("Modifying file in wc2 (creating conflict)...");
    let file2 = wc2_path.join("test.txt");
    std::fs::write(&file2, "Line 1\nModified by WC2\nLine 3\n")?;

    // Update wc2 - this will create a conflict
    println!("Updating wc2 (conflict will occur)...");
    let _ = ctx.update(
        &[wc2_path.to_str().unwrap()],
        Revision::Head,
        &UpdateOptions {
            depth: Depth::Infinity,
            depth_is_sticky: false,
            ignore_externals: false,
            allow_unver_obstructions: false,
            adds_as_modifications: false,
            make_parents: false,
        },
    );

    // Now demonstrate the conflict resolution API
    println!("\n=== Demonstrating Conflict Resolution API ===\n");

    // 1. Get the conflict
    println!("1. Getting conflict information...");
    let mut conflict = ctx.conflict_get(&file2)?;

    // 2. Check what types of conflicts exist
    let (has_text, prop_conflicts, has_tree) = conflict.get_conflicted()?;
    println!("   Text conflict: {}", has_text);
    println!("   Property conflicts: {:?}", prop_conflicts);
    println!("   Tree conflict: {}", has_tree);

    if has_text {
        // 3. Get conflict details
        println!("\n2. Examining text conflict details...");
        let local_abspath = conflict.get_local_abspath();
        println!("   Conflicted file: {}", local_abspath);

        let mime_type = conflict.text_get_mime_type()?;
        println!("   MIME type: {:?}", mime_type);

        let incoming_change = conflict.get_incoming_change();
        let local_change = conflict.get_local_change();
        println!("   Incoming change: {:?}", incoming_change);
        println!("   Local change: {:?}", local_change);

        // 4. Get available resolution options
        println!("\n3. Getting resolution options...");
        let options = conflict.text_get_resolution_options(&mut ctx)?;
        println!("   Available options:");
        for (i, opt) in options.iter().enumerate() {
            let id = opt.get_id();
            let label = opt.get_label();
            let desc = opt.get_description();
            println!("   {}. {} - {} (ID: {:?})", i + 1, label, desc, id);
        }

        // 5. Find a specific option by ID
        println!("\n4. Finding 'merged text' option...");
        let merged_option = subversion::client::ConflictOption::find_by_id(
            &options,
            subversion::ClientConflictOptionId::MergedText,
        );

        if let Some(opt) = merged_option {
            println!("   Found option: {}", opt.get_label());

            // 6. Resolve the conflict using the merged text option
            println!("\n5. Resolving conflict with merged text...");
            conflict.text_resolve_by_id(subversion::TextConflictChoice::Merged, &mut ctx)?;

            let resolution = conflict.text_get_resolution();
            println!("   Resolution applied: {:?}", resolution);
        } else {
            // Alternative: use working text
            println!("\n5. Resolving conflict with working version...");
            conflict.text_resolve_by_id(
                subversion::TextConflictChoice::MineFull,
                &mut ctx,
            )?;
        }

        println!("\n✓ Conflict resolved successfully!");
    }

    // Example with property conflicts
    println!("\n=== Property Conflict Example ===\n");
    println!("Creating property conflict...");

    // Set a property
    ctx.propset(
        "custom:prop",
        Some(b"value1"),
        file1.to_str().unwrap(),
        &subversion::client::PropSetOptions::default(),
    )?;

    ctx.commit(
        &[wc1_path.to_str().unwrap()],
        &CommitOptions::default(),
        std::collections::HashMap::new(),
        &|_| Ok(()),
    )?;

    // Set different value in wc2
    ctx.propset(
        "custom:prop",
        Some(b"value2"),
        file2.to_str().unwrap(),
        &subversion::client::PropSetOptions::default(),
    )?;

    // Update to create conflict
    let _ = ctx.update(
        &[wc2_path.to_str().unwrap()],
        Revision::Head,
        &UpdateOptions::default(),
    );

    // Get property conflict
    if let Ok(mut prop_conflict) = ctx.conflict_get(&file2) {
        let (_, props, _) = prop_conflict.get_conflicted()?;
        if !props.is_empty() {
            println!("Property conflict detected on: {:?}", props);

            // Get property values
            let propvals = prop_conflict.prop_get_propvals(&props[0])?;
            println!("  Base value: {:?}", propvals.0);
            println!("  Working value: {:?}", propvals.1);
            println!("  Incoming old value: {:?}", propvals.2);
            println!("  Incoming new value: {:?}", propvals.3);

            // Get resolution options
            let prop_options = prop_conflict.prop_get_resolution_options(&mut ctx)?;
            println!("  Available property resolution options: {}", prop_options.len());

            // Resolve by choosing working version
            prop_conflict.prop_resolve_by_id(
                &props[0],
                subversion::TextConflictChoice::MineFull,
                &mut ctx,
            )?;
            println!("  ✓ Property conflict resolved");
        }
    }

    println!("\n=== Example completed successfully! ===");
    println!("\nThis example demonstrated:");
    println!("  • Getting conflict information with conflict_get");
    println!("  • Examining conflict details and types");
    println!("  • Listing available resolution options");
    println!("  • Finding specific options by ID");
    println!("  • Resolving text and property conflicts");
    println!("  • Using prop_get_propvals to examine property values");

    Ok(())
}

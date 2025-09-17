use std::io::Write;
use subversion::fs::Fs;
use subversion::Revnum;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary filesystem for demonstration
    let temp_dir = tempfile::tempdir()?;
    let fs_path = temp_dir.path().join("test-fs");

    println!("Creating filesystem at: {:?}", fs_path);
    let fs = Fs::create(&fs_path)?;

    // Begin a transaction
    println!("Beginning transaction...");
    let mut txn = fs.begin_txn(Revnum::from(0u32), 0)?;

    // Get transaction information
    let txn_name = txn.name()?;
    let base_rev = txn.base_revision()?;
    println!("Transaction name: {}", txn_name);
    println!("Base revision: {:?}", base_rev);

    // Set transaction properties (commit message, author)
    println!("Setting transaction properties...");
    txn.change_prop("svn:log", "Demo commit message")?;
    txn.change_prop("svn:author", "demo-user")?;

    // Get the transaction root to make changes
    println!("Making filesystem changes...");
    let mut root = txn.root()?;

    // Create a directory structure
    root.make_dir("trunk")?;
    root.make_dir("trunk/src")?;

    // Create some files
    root.make_file("trunk/README.txt")?;
    root.make_file("trunk/src/main.rs")?;

    // Add content to files
    let mut stream = root.apply_text("trunk/README.txt", None)?;
    stream.write_all(b"This is a demo repository.\n")?;
    stream.write_all(b"Created using Subversion Rust bindings.\n")?;
    drop(stream);

    let mut stream = root.apply_text("trunk/src/main.rs", None)?;
    stream.write_all(b"fn main() {\n")?;
    stream.write_all(b"    println!(\"Hello from SVN!\");\n")?;
    stream.write_all(b"}\n")?;
    drop(stream);

    // Set properties on the files
    root.change_node_prop("trunk/README.txt", "svn:mime-type", b"text/plain")?;
    root.change_node_prop("trunk/src/main.rs", "svn:mime-type", b"text/x-rustsrc")?;
    root.change_node_prop("trunk/src/main.rs", "custom:language", b"rust")?;

    // Commit the transaction
    println!("Committing transaction...");
    let new_rev = txn.commit()?;
    println!("Successfully committed as revision: {:?}", new_rev);

    // Verify the filesystem state
    let youngest = fs.youngest_revision()?;
    println!("Filesystem youngest revision: {:?}", youngest);

    // Get the committed root to verify our changes
    let committed_root = fs.revision_root(new_rev)?;
    let trunk_kind = committed_root.check_path("trunk")?;
    println!("trunk/ exists as: {:?}", trunk_kind);

    let readme_kind = committed_root.check_path("trunk/README.txt")?;
    println!("trunk/README.txt exists as: {:?}", readme_kind);

    // Show properties
    let props = committed_root.proplist("trunk/src/main.rs")?;
    println!("Properties on trunk/src/main.rs:");
    for (key, value) in props {
        println!("  {}: {}", key, String::from_utf8_lossy(&value));
    }

    println!("Demo completed successfully!");
    Ok(())
}

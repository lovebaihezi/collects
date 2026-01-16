//! Schema information command.

use crate::output::Output;

/// Prints schema information about what can be added to collects.
pub fn print_schema() {
    let out = Output::new();

    out.header("Collects Content Schema");
    out.divider(24);
    out.newline();

    out.print("When creating or adding content to a collect, you can provide:");
    out.newline();

    out.section("TITLE/DESCRIPTION");
    out.section_content("Not settable via CLI; titles come from filenames or defaults.");
    out.newline();

    out.section("BODY (optional)");
    out.section_content("Text content, provided via:");
    out.section_content("- stdin: echo 'content' | collects new -t 'My Collect' --stdin");
    out.section_content("- stdin (add): echo 'content' | collects add <collect_id> --stdin");
    out.newline();

    out.section("ATTACHMENTS (optional)");
    out.section_content("Files to upload with the content:");
    out.section_content("- File flag: --file, -f <PATH> (can be repeated)");
    out.section_content("- Clipboard: Images in clipboard are automatically attached");
    out.newline();

    out.subheader("Examples:");
    out.newline();

    out.example(
        "Create a collect with text from stdin",
        "echo 'My note content' | collects new -t 'My Collect' --stdin",
    );
    out.newline();

    out.example(
        "Add a file to an existing collect",
        "collects add <collect_id> -f image.png",
    );
    out.newline();

    out.example(
        "Paste from clipboard (image) into a new collect",
        "collects new -t 'Clipboard image'",
    );
    out.newline();

    out.example(
        "Multiple files",
        "collects new -t 'Multiple files' -f file1.txt -f file2.png",
    );
}

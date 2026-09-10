import CodeBlock from "@theme/CodeBlock";

// A line holding only this, after its comment marker, opens a hidden region.
const OPEN = "<";

// And this one closes it.
const CLOSE = ">";

// The marker is written as a comment so the file it lives in still compiles
// and still parses. Rust and TOML disagree about which leader that is, and
// this component is pointed at both.
const LEADERS = ["#", "//"];

/**
 * Returns the marker a line carries, or null when it carries none.
 */
function marker(line) {
  const text = line.trim();

  for (const leader of LEADERS) {
    if (text.startsWith(leader)) {
      const rest = text.slice(leader.length).trim();
      if (rest === OPEN || rest === CLOSE) {
        return rest;
      }
    }
  }

  return null;
}

/**
 * Renders an imported file, minus the regions its markers hide.
 *
 * The page imports the real file, so what a reader copies is what CI
 * compiles. Some of that file is there for the repository rather than for
 * the reader: a manifest comment about workspace membership, say. Wrapping
 * those lines in markers keeps them in the file and off the page.
 */
export default function FilteredCodeBlock({ children, ...props }) {
  const shown = [];
  let hidden = false;

  for (const line of String(children).split("\n")) {
    const found = marker(line);

    if (found === OPEN) {
      hidden = true;
    } else if (found === CLOSE) {
      hidden = false;
    } else if (!hidden) {
      shown.push(line);
    }
  }

  return <CodeBlock {...props}>{shown.join("\n").trim()}</CodeBlock>;
}

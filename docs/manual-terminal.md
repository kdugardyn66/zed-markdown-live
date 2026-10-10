---
title: Using markdown-zed in the Terminal — manual
---
# Using `markdown-zed` in the Terminal

*Manual for MarkDown Live 0.7.1.* MarkDown Live is one program, **`markdown-zed`**. Zed
starts it in the background, but you can also use it yourself in the Terminal: to preview
or print any Markdown file, to check your setup, or to repair the Zed keys. Each command
below is followed by what you see.

[TOC]

## 1. Before you start

### 1.1 Where the program is

`./setup.sh` (in the MarkDown Live folder) installs it as `~/.cargo/bin/markdown-zed`.
Check that the Terminal finds it:

```bash
markdown-zed --version
```

```text
markdown-live 0.7.1
```

If you get `command not found: markdown-zed`, add the folder to your PATH once and try
again:

```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

### 1.2 File names with spaces

Put quotes around paths with spaces (iCloud folders have them), or drag the file from
Finder into the Terminal window, which adds the path for you:

```bash
markdown-zed preview "$HOME/Library/Mobile Documents/com~apple~CloudDocs/Notes/plan.md"
```

### 1.3 All commands at a glance

```bash
markdown-zed --help
```

```text
markdown-live 0.7.1

USAGE:
  markdown-zed [--stdio]                  language server (used by Zed)
  markdown-zed preview FILE [--line N] [--new-window]
  markdown-zed render FILE                print the rendered HTML
  markdown-zed serve                      standalone preview server
  markdown-zed doctor                     show the detected setup
  markdown-zed setup-zed [--key alt-m]    add the preview task + key binding to Zed
  markdown-zed print FILE                 open the preview and show the print dialog
  markdown-zed insert-slide FILE --line N insert <!-- slide --> (in Zed: through the language server)

Environment: MARKDOWN_LIVE_SETTINGS (JSON, same keys as the Zed settings), MARKDOWN_LIVE_CHROME
```

| Command | Use it to |
|---|---|
| `preview` | open the live preview of a file |
| `print` | print a file, or save it as PDF |
| `insert-slide` | add a slide marker to a file |
| `render` | see the HTML MarkDown Live makes (for checking) |
| `doctor` | check what MarkDown Live found on your Mac |
| `setup-zed` | (re)create the Zed keys Option-M, Option-Shift-N, Option-Shift-P |
| `serve` | start a preview server by hand (normally automatic) |

## 2. Preview a file

```bash
markdown-zed preview notes.md
```

```text
MarkDown Live preview: http://127.0.0.1:58428/?t=4259a75a7deba16ef52fec95adcc5ea8
```

The preview opens in your browser and the address is printed. What it shows depends on
whether Zed has the file open:

- **Zed has it open:** the preview shows what is in Zed, also text you haven't saved,
  and follows your cursor. This is exactly what **Option-M** does.
- **Zed doesn't:** a small preview server starts in the background and shows the file
  on disk. Every time the file is saved (by any editor) the preview updates. The server
  stops by itself two minutes after you close the preview.

Options:

| Option | Meaning | Example |
|---|---|---|
| `--line N` | start at line N (scroll there) | `markdown-zed preview notes.md --line 120` |
| `--new-window` | always open a new browser window, even when a preview is already open | `markdown-zed preview notes.md --new-window` |

Opening another file reuses the preview window that's already open: it switches to the
new file.

## 3. Print a file or save it as PDF

```bash
markdown-zed print report.md
```

The preview opens (if it wasn't open yet) and shows the browser's print dialog for the
document. Choose a printer, or **PDF → Save as PDF**. This is what **Option-Shift-P**
does in Zed.

- The printout is in the light theme, without the preview's toolbar.
- A document with `<!-- slide -->` markers prints one slide per page: in **portrait**
  with the notes under the slide, in **landscape** with the notes on the next page.
- Videos are left out; charts and 3D molecules are printed as pictures.

If nothing seems to happen, the dialog is in the preview window behind other windows:
bring the browser to the front.

## 4. Add a slide marker

```bash
markdown-zed insert-slide talk.md --line 12
```

Inserts a line `<!-- slide -->` so that it becomes line 12 of `talk.md` (the old line 12
moves down). If Zed has the file open, the change goes through Zed (so you can undo it
with Cmd-Z); otherwise the file is changed on disk.

Before (lines 10–12):

```markdown
- print it

## Questions?
```

After `--line 12`:

```markdown
- print it

<!-- slide -->
## Questions?
```

## 5. Check your setup: `doctor`

```bash
markdown-zed doctor
```

Example on a Mac:

```text
markdown-live 0.7.1
browser       : system default (no Chromium/Chrome found)
libraries     : bundled (50 embedded files)
instances     : /Users/duggie/Library/Caches/markdown-live/instances
running       : port 58428 (lsp), 1 open file(s)
math          : katex
plantuml      : local (/opt/homebrew/opt/openjdk/bin/java -jar /opt/homebrew/opt/plantuml/libexec/plantuml.jar)
  java        : /opt/homebrew/opt/openjdk/bin/java
  jar         : /opt/homebrew/opt/plantuml/libexec/plantuml.jar
kroki         : https://kroki.io (only for ```kroki-…``` blocks)
ffmpeg        : /opt/homebrew/bin/ffmpeg
code chunks   : enabled (python: /Users/duggie/.venvs/mypythonenv/bin/python3)
```

How to read it:

| Line | Meaning | If it's not what you expect |
|---|---|---|
| `browser` | where the preview opens | install Chromium/Chrome for a separate app window, or set `"browser"` |
| `running` | preview servers now running: **lsp** = started by Zed, **standalone** = started from the Terminal | a *standalone* one while working in Zed: `pkill -f "markdown-zed serve"` |
| `plantuml`, `java`, `jar` | how PlantUML diagrams are drawn | `brew install plantuml`, or set `"plantumlJar"` / `"java"` |
| `ffmpeg` | converts videos the browser can't play | `brew install ffmpeg` |
| `code chunks` | whether `{cmd=true}` code runs, and with which Python | set `"enableScriptExecution"` and `"python"` in Zed's settings |

`doctor` uses the same settings as Zed (the `lsp` → `markdown-live` → `settings` part of
Zed's settings.json).

## 6. Repair the Zed keys: `setup-zed`

```bash
markdown-zed setup-zed
```

```text
updated /Users/duggie/.config/zed/tasks.json
updated /Users/duggie/.config/zed/keymap.json
Done. In Zed, open a Markdown file and press Alt-M to open the live preview.
  Alt-Shift-N → MarkDown Live: Insert new slide
  Alt-Shift-P → MarkDown Live: Print
Or: command palette → "task: spawn" → "MarkDown Live: Preview".
```

(Alt is the Option key.) It only changes its own part of those files, between the lines
`// >>> markdown-live` and `// <<< markdown-live`; your other keys and tasks stay as they
are. To use another key for the preview:

```bash
markdown-zed setup-zed --key alt-k
```

`./setup.sh` runs this for you after installing a new version.

## 7. See the HTML: `render`

```bash
markdown-zed render notes.md
```

For a file with

```markdown
# Hello

Some *text*.
```

it prints:

```text
<h1 id="hello" data-sourcepos="1:1-1:7">Hello<a href="#hello" aria-label="Link to heading 'Hello'" data-heading-content="Hello" class="anchor"></a></h1>
<p data-sourcepos="3:1-3:12">Some <em data-sourcepos="3:6-3:11">text</em>.</p>
```

This is for checking what MarkDown Live makes of your Markdown (or for reporting a
problem). It is not a finished web page: diagrams, charts and formulas are only drawn
in the preview. To keep it in a file: `markdown-zed render notes.md > notes.html`.
PlantUML diagrams and code chunks are run while rendering, so it can take a moment.

## 8. Settings from the Terminal

From the Terminal, `markdown-zed` uses your Zed settings (`lsp` → `markdown-live` →
`settings` in Zed's settings.json). To change a setting for one command, put
`MARKDOWN_LIVE_SETTINGS` in front of it, with the settings in JSON:

```bash
# dark preview, just this once
MARKDOWN_LIVE_SETTINGS='{"theme":"dark"}' markdown-zed preview notes.md

# run code chunks without asking, with another Python
MARKDOWN_LIVE_SETTINGS='{"enableScriptExecution":true,"python":"/usr/bin/python3"}' markdown-zed render analysis.md > out.html
```

`MARKDOWN_LIVE_CHROME=/path/to/browser` picks the Chromium-type browser for the preview
window. Note: a preview server that is already running keeps its settings; stop it
first (`pkill -f "markdown-zed serve"`) if a setting doesn't seem to apply.

## 9. Preview servers

You normally never start one yourself, but it helps to know what runs:

| Server | Started by | Settings | Shows |
|---|---|---|---|
| **lsp** | Zed, when you open a Markdown file | Zed's settings | your text in Zed, also unsaved |
| **standalone** | `markdown-zed preview/print` when Zed doesn't have the file open (or `markdown-zed serve`) | Zed's settings.json + `MARKDOWN_LIVE_SETTINGS` | the file on disk |

The footer of the preview says which one you're looking at (**live (editor)** or
**standalone**). Useful commands:

```bash
pgrep -fl markdown-zed               # what is running
pkill -f "markdown-zed serve"        # stop the standalone servers
```

To restart the one Zed uses: in Zed, command palette (Cmd-Shift-P) → **editor: restart
language server**.

## 10. Recipes

**Preview a file from Finder's folder in the Terminal**

```bash
cd ~/Documents/Notes
markdown-zed preview plan.md
```

**Make a PDF of a document**

```bash
markdown-zed print report.md
# in the dialog: PDF → Save as PDF
```

**Check a setup problem and send the details to someone**

```bash
markdown-zed doctor > ~/Desktop/markdown-live-doctor.txt
```

**Open the preview at the line you're working on, from a script**

```bash
markdown-zed preview chapter3.md --line 250
```

**Install a new version (from the MarkDown Live folder)**

```bash
git fetch ../markdown-live-0.x.y.bundle 'refs/tags/*:refs/tags/*'
git checkout 0.x.y
./setup.sh
pkill -f "markdown-zed serve"
# then in Zed: Extensions → Rebuild MarkDown Live, and "editor: restart language server"
```

## 11. Problems

| Message or symptom | Fix |
|---|---|
| `command not found: markdown-zed` | see 1.1 (PATH) |
| `no such file: …` | check the path; put quotes around paths with spaces |
| `could not start the preview server` | run `markdown-zed doctor`; another program may block local ports |
| preview shows an old version of the file | it's a standalone server showing the file on disk: save the file, or open it in Zed |
| "code execution is off" or the wrong Python | `markdown-zed doctor` → `code chunks`; fix the settings, then `pkill -f "markdown-zed serve"` |
| the print dialog doesn't appear | it is in the browser window; bring it to the front |

See also the manual *Writing Markdown in Zed with MarkDown Live* (`manual-markdown.md`).

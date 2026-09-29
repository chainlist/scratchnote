# Plugin registry, as a folder

This folder stands in for GitHub while the community plugin list is not
published (SPEC 4.10). A dev build of Scratchnote reads it instead of GitHub;
any build does when `SCRATCHNOTE_PLUGIN_REGISTRY` names a folder laid out like
this one.

It mirrors the URLs the app asks GitHub for, host first, so it works exactly
as the real registry will:

| The app asks for                                               | This folder answers with                               |
| -------------------------------------------------------------- | ------------------------------------------------------ |
| `https://raw.githubusercontent.com/<repo>/HEAD/<file>`         | `raw.githubusercontent.com/<repo>/HEAD/<file>`         |
| `https://github.com/<repo>/releases/download/<version>/<file>` | `github.com/<repo>/releases/download/<version>/<file>` |

- `raw.githubusercontent.com/chainlist/scratchnote-plugins/HEAD/community-plugins.json`
  is the list of plugins, as Obsidian's `obsidian-releases` repository has:
  each plugin's id, name, author, description and repository.
- `raw.githubusercontent.com/<repo>/HEAD/` is a plugin's repository: its
  `manifest.json`, whose version is the latest release, and its `README.md`,
  shown before it is installed.
- `github.com/<repo>/releases/download/<version>/` is the GitHub release of
  that version, with the files that are installed: `manifest.json`,
  `main.js` and, if the plugin has one, `styles.css`.

To publish for real, make `chainlist/scratchnote-plugins` hold
`community-plugins.json`, give each plugin a repository with `manifest.json`
and `README.md` at its root, and attach the files to a GitHub release tagged
with the version. Nothing in the app changes.

The list is empty for now: its two example plugins became the Mentions and
Stats core plugins, in `src/plugins/`, which show the API as well
(PLUGINS.md). To try installing, add a plugin here laid out as above.

## Decisions

Retain the existing Base UI Select, portal, groups and folder button. Render an explanatory block only when there are no options, rather than a disabled fake repository. Constrain its text width and pad it so it wraps on a narrow window. The selector has the accessible name Repository and describes itself with the message while open. Hosts without a folder picker direct users to their IDE project.

The empty state means no available choices, not a discovery or loading failure. Once a repository is opened, the normal options replace the explanation. Document the empty popup in UI Theme.

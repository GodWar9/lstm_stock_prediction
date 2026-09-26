# Research inspector design

The interface is a calm laboratory notebook for inspecting evidence, with the
split timeline as its strongest visual element.

## Palette and type

- Paper `#f3f6fa`: workspace background.
- Ink `#182b49`: primary text and navigation.
- Cobalt `#2454cb`: selected run, links and held-out test windows.
- Teal `#147d72`: positive values, training windows.
- Rose `#b83352`: negative values and errors.
- Amber `#8b610b`: uncertainty, purge and embargo warnings.

System sans-serif is used for labels and prose; the system monospace stack is
reserved for artifact identifiers. Sizes: 12, 14, 16, 22 and 34px. Numeric values
use tabular figures. Positive/negative values retain explicit signs. Split
windows have text labels; gaps also use hatching. Color is never the only cue.

## Layout

Persistent navigation and an evidence footer surround a wide inspection canvas.

```text
Navigation | Research inspector             Run selector
           | Active instrument / model / split / source
           | Sharpe    Drawdown    Return    Turnover
           | [ wide equity plot + accessible table ]
           | Warnings and methodology
-----------+-------------------------------------------------
           | Copyable run / dataset / model / commit / config
```

```text
Data and validation
Dataset version       PIT checks       Scaler boundary
Fold 1 [ TRAIN ////// PURGE \\\\ EMBARGO | VALIDATION | ... | TEST ]
       Start and end indices, UTC dates, and explanations
Exact recorded validation JSON available below
```

## Principles and review

Use source labels, exact units, capability-driven empty states, and provenance
instead of decorative dashboard cards. Charts display Rust results; the browser
does not calculate returns or metrics. Out-of-sample runs are the default and
in-sample inspection requires an explicit toggle. Unverified results and
synthetic data are visibly identified.

The initial dark trading-terminal convention was rejected: a slate and blue
light canvas improves document and timeline reading. No gradients, repeating
shadows, all-caps eyebrows, or entrance animations. Mobile navigation wraps,
keyboard focus is visible, and every plot has a tabular alternative.

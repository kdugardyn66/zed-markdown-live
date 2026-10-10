# 3D molecules

MarkDown Live shows 3D structures with [Mol*](https://molstar.org), the viewer used by
the Protein Data Bank. Drag to rotate, scroll to zoom, right-drag to move; the buttons
top right reset the view, take a screenshot or show it full screen.

## From a file

@import "molecules/caffeine.sdf" {height=320 title="Caffeine (SDF)"}

@import "molecules/peptide.pdb" {height=360 title="A small peptide (PDB)" spin=true}

Supported files: `.pdb`, `.cif`/`.mmcif`, `.bcif`, `.sdf`, `.mol`, `.mol2`, `.xyz`,
`.gro`, `.pdbqt`. Options: `height`, `title`, `spin=true`.

## Pasted into the document

```xyz {height=260 title="Water (XYZ)"}
3
water
O 0.000 0.000 0.117
H 0.000 0.757 -0.469
H 0.000 -0.757 -0.469
```

Use the format as the block language (```` ```pdb ````, ```` ```sdf ````, ```` ```mol2 ```` …),
or ```` ```molstar ```` to let MarkDown Live detect it. Add `{code=true}` to show the text
as code instead.

## From the Protein Data Bank (online)

This is the only molecule block that goes online: it downloads the entry from the RCSB PDB.

````markdown
```molstar {pdb=1CRN height=400}
```
````

## 2D structures

For 2D drawings from SMILES, use ```` ```smiles ```` (see `v2-diagrams.md`):

```smiles
CN1C=NC2=C1C(=O)N(C(=O)N2C)C caffeine
```

## Code that draws a graph

```python {cmd=true hide=true}
import matplotlib.pyplot as plt
x = list(range(0, 11))
plt.plot(x, [v * v for v in x], marker="o")
plt.title("x²")
plt.show()
```

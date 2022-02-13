# Wat

## Library

### withPkgsFor
```
:: [ String ] -- ^ List of System names (e.g. "x68_64-linux")
-> Nixpkgs -- ^ Flake output of a nixpkgs-like flake
-> [ Overlay ] -- ^ List of overlays for the package set
-> ( AttrSet -> Any ) -- ^ Function that takes a "pkgs" input and may return anything
-> AttrSet -- ^ Attrset with the systems as keys and the values being the outputs of the function
```

### withPkgsForLinux
```
:: Nixpkgs -- ^ Flake output of a nixpkgs-like flake
-> [ Overlay ] -- ^ List of overlays for the package set
-> ( AttrSet -> Any ) -- ^ Function that takes a "pkgs" input and may return anything
-> AttrSet -- ^ Attrset with the systems as keys and the values being the outputs of the function
```

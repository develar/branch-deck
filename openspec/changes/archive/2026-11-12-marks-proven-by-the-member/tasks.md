# Tasks

- [x] 1.1 A rename of a local is proven by the member around it (`Renames::prove` over `NameAt::statements`), a
  line without code is no change of a name, with a table test of the cases the review found.
- [x] 1.2 The proofs read the blocks of the declared names with those names as one (`with_declarations`): a
  function renamed with a local of its body leaves the local proven.
- [x] 1.3 A name that is not local says whether other code declares it (`read_declarations`,
  `Declarations::redeclared`); the subject asks no index of it and names `otherDeclaration`, with a table test.
- [x] 1.4 The place of a C# local in an argument is read (`a_place_that_makes_more_of_its_name_than_its_binding_is_read`).
- [x] 1.5 `MODEL_VERSION` 17 with the new layout of what a pair leaves.

// The species drawn in 3D, by the app's species name. Apart from rig.ts so the mesh worker can load
// them without the page's frame code.
import type { SpeciesDef } from "./species";
import { cat } from "./cat";
import { dog } from "./dog";
import { hamster } from "./hamster";
import { macaw } from "./macaw";

export const SPECIES_3D: Record<string, SpeciesDef> = { cat, dog, hamster, parrot: macaw };

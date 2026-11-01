// Which rig draws each species. An unknown species falls back to the cat.
import catUrl from "../../art/cat/build/cat.riv?url";
import dogUrl from "../../art/dog/build/dog.riv?url";
import hamsterUrl from "../../art/hamster/build/hamster.riv?url";
import parrotUrl from "../../art/parrot/build/parrot.riv?url";

const ART: Record<string, string> = { cat: catUrl, dog: dogUrl, hamster: hamsterUrl, parrot: parrotUrl };

export function artFor(species: string): string {
  return ART[species] ?? catUrl;
}

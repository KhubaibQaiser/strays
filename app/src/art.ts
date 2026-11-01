// Which rig draws each species. A species without its own art yet falls back to the cat.
import catUrl from "../../art/cat/build/cat.riv?url";
import dogUrl from "../../art/dog/build/dog.riv?url";
import hamsterUrl from "../../art/hamster/build/hamster.riv?url";

const ART: Record<string, string> = { cat: catUrl, dog: dogUrl, hamster: hamsterUrl };

export function artFor(species: string): string {
  return ART[species] ?? catUrl;
}

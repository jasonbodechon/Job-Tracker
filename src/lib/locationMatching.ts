const canadaMarkers = [
  "canada", "alberta", "ab", "british columbia", "bc", "manitoba", "mb", "new brunswick", "nb",
  "newfoundland and labrador", "nl", "nova scotia", "ns", "northwest territories", "nt", "nunavut", "nu",
  "ontario", "on", "prince edward island", "pe", "quebec", "qc", "saskatchewan", "sk", "yukon", "yt",
  "toronto", "ottawa", "kitchener", "waterloo", "vancouver", "montreal", "calgary", "edmonton", "winnipeg",
  "halifax", "saskatoon", "regina", "mississauga", "brampton", "markham", "burnaby", "surrey",
];

const unitedStatesMarkers = [
  "united states", "united states of america", "usa", "us", "alabama", "al", "alaska", "ak", "arizona", "az",
  "arkansas", "ar", "california", "ca", "colorado", "co", "connecticut", "ct", "delaware", "de",
  "district of columbia", "dc", "florida", "fl", "georgia", "ga", "hawaii", "hi", "idaho", "id",
  "illinois", "il", "indiana", "in", "iowa", "ia", "kansas", "ks", "kentucky", "ky", "louisiana", "la",
  "maine", "me", "maryland", "md", "massachusetts", "ma", "michigan", "mi", "minnesota", "mn",
  "mississippi", "ms", "missouri", "mo", "montana", "mt", "nebraska", "ne", "nevada", "nv",
  "new hampshire", "nh", "new jersey", "nj", "new mexico", "nm", "new york", "ny", "north carolina", "nc",
  "north dakota", "nd", "ohio", "oh", "oklahoma", "ok", "oregon", "or", "pennsylvania", "pa",
  "rhode island", "ri", "south carolina", "sc", "south dakota", "sd", "tennessee", "tn", "texas", "tx",
  "utah", "ut", "vermont", "vt", "virginia", "va", "washington", "wa", "west virginia", "wv",
  "wisconsin", "wi", "wyoming", "wy",
];

export function normalizeLocation(value: string): string {
  return ` ${value.toLocaleLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim().replace(/\s+/g, " ")} `;
}

function containsLocationTerm(location: string, value: string): boolean {
  const term = normalizeLocation(value);
  return term.trim().length > 0 && location.includes(term);
}

function countryMatches(location: string, country: string): boolean {
  const countryName = normalizeLocation(country).trim();
  if (!countryName) return false;
  if (containsLocationTerm(location, countryName)) return true;
  const markers = ["canada", "canadian"].includes(countryName)
    ? canadaMarkers
    : ["united states", "united states of america", "usa", "us", "america"].includes(countryName)
      ? unitedStatesMarkers
      : [];
  return markers.some((marker) => containsLocationTerm(location, marker));
}

export function locationGroupMatches(location: string, values: string[], countries = false): boolean {
  const configured = values.map((value) => value.trim()).filter(Boolean);
  return configured.length === 0 || configured.some((value) => countries ? countryMatches(location, value) : containsLocationTerm(location, value));
}

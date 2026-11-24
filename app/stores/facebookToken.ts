/** Where Meta's OAuth redirect puts the user token, longest-lived first. */
const TOKEN_PARAMS = [
  "long_lived_token",
  "long_lived_access_token",
  "long_token",
  "fb_exchange_token",
  "provider_token",
  "access_token",
] as const;

/**
 * The access token in what the user pasted: a bare token, or the whole redirect URL (the token is
 * in its query or fragment). `null` when there is none, so the form can say what is missing.
 */
export function facebookTokenFromInput(raw: string): string | null {
  const trimmed = raw.trim();
  if (trimmed === "") return null;
  if (!trimmed.startsWith("http://") && !trimmed.startsWith("https://")) return trimmed;
  for (const name of TOKEN_PARAMS) {
    const match = trimmed.match(new RegExp(`[#&?]${name}=([^&]+)`));
    if (match?.[1]) {
      try {
        return decodeURIComponent(match[1]);
      } catch {
        return match[1];
      }
    }
  }
  return null;
}

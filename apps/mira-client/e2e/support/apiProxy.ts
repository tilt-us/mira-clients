import type { Page, Route } from "@playwright/test";

const proxiedApiOrigins =
  /^(https:\/\/api\.tilt-us\.com|http:\/\/localhost:808[0-3])\//;

export async function proxyApiRequests(page: Page) {
  await page.route(proxiedApiOrigins, async (route) => {
    await proxyApiRequest(route);
  });
}

async function proxyApiRequest(route: Route) {
  const request = route.request();
  const requestHeaders = request.headers();
  const origin = requestHeaders.origin ?? "http://127.0.0.1:4173";
  const corsHeaders = {
    "access-control-allow-credentials": "true",
    "access-control-allow-headers":
      requestHeaders["access-control-request-headers"] ??
      "authorization, content-type",
    "access-control-allow-methods":
      requestHeaders["access-control-request-method"] ??
      "GET, POST, PUT, PATCH, DELETE, OPTIONS",
    "access-control-allow-origin": origin,
    "access-control-expose-headers": "*",
    vary: "Origin",
  };

  if (request.method() === "OPTIONS") {
    await route.fulfill({
      body: "",
      headers: corsHeaders,
      status: 204,
    });
    return;
  }

  const response = await route.fetch({
    headers: removeBrowserOriginHeaders(requestHeaders),
  });

  await route.fulfill({
    response,
    headers: {
      ...response.headers(),
      ...corsHeaders,
    },
  });
}

function removeBrowserOriginHeaders(headers: Record<string, string>) {
  const cleanHeaders = { ...headers };

  for (const header of [
    "origin",
    "referer",
    "sec-fetch-dest",
    "sec-fetch-mode",
    "sec-fetch-site",
    "sec-fetch-user",
  ]) {
    delete cleanHeaders[header];
  }

  return cleanHeaders;
}

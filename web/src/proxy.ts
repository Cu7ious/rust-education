import createMiddleware from "next-intl/middleware";
import { NextResponse, type NextRequest } from "next/server";

import { routing } from "./i18n/routing";

const intl = createMiddleware(routing);

export function proxy(request: NextRequest) {
  const { pathname } = request.nextUrl;
  const locale = routing.locales.find(
    (item) => pathname === `/${item}` || pathname.startsWith(`/${item}/`),
  );
  const rest = locale ? pathname.slice(locale.length + 1) : pathname;
  const needsSession =
    rest.startsWith("/courses") || rest.startsWith("/lessons");
  if (
    locale &&
    needsSession &&
    !request.cookies.get("edurust_session")?.value
  ) {
    const url = request.nextUrl.clone();
    url.pathname = `/${locale}/login`;
    url.searchParams.set("next", pathname);
    return NextResponse.redirect(url);
  }
  return intl(request);
}

export const config = {
  matcher: ["/((?!api|_next|.*\\..*).*)"],
};

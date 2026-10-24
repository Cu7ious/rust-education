"use client";

import { useLocale, useTranslations } from "next-intl";
import { useCallback, useEffect, useRef, useState } from "react";

import { SlideToast } from "@/components/slide-toast";
import { Button, buttonVariants } from "@/components/ui/button";
import { Link, usePathname, useRouter } from "@/i18n/navigation";
import { ApiError, api, type UserResponse } from "@/lib/api/client";

const LOGOUT_NOTICE = "edurust.toast";

export function SiteHeader() {
  const t = useTranslations("nav");
  const locale = useLocale();
  const pathname = usePathname();
  const router = useRouter();
  const [loggedIn, setLoggedIn] = useState(false);
  const [busy, setBusy] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const dismissToast = useCallback(() => setToast(null), []);
  const logoutStartedHere = useRef(false);

  useEffect(() => {
    let active = true;
    api<UserResponse>("/auth/me")
      .then(() => {
        if (active) setLoggedIn(true);
      })
      .catch(() => {
        if (active) setLoggedIn(false);
      });
    return () => {
      active = false;
    };
  }, [pathname]);

  useEffect(() => {
    if (logoutStartedHere.current) {
      return;
    }
    if (sessionStorage.getItem(LOGOUT_NOTICE) !== "loggedOut") {
      return;
    }
    const id = window.setTimeout(() => {
      sessionStorage.removeItem(LOGOUT_NOTICE);
      setToast(t("loggedOut"));
    }, 0);
    return () => window.clearTimeout(id);
  }, [pathname, t]);

  async function logout() {
    setBusy(true);
    try {
      await api("/auth/logout", { method: "POST", csrf: true });
    } catch (error) {
      if (!(error instanceof ApiError) || error.status !== 401) {
        setBusy(false);
        return;
      }
    }
    setLoggedIn(false);
    logoutStartedHere.current = true;
    sessionStorage.setItem(LOGOUT_NOTICE, "loggedOut");
    setBusy(false);
    router.replace("/");
  }

  return (
    <header className="border-b">
      <div className="mx-auto flex max-w-3xl items-center justify-between gap-3 px-4 py-3">
        <Link href="/" className="font-heading text-base font-medium">
          Edurust
        </Link>
        <nav className="flex items-center gap-2">
          <Link
            href={pathname}
            locale="en"
            className={buttonVariants({
              variant: locale === "en" ? "secondary" : "ghost",
              size: "sm",
            })}
          >
            EN
          </Link>
          {/*
          <Link
            href={pathname}
            locale="uk"
            className={buttonVariants({
              variant: locale === "uk" ? "secondary" : "ghost",
              size: "sm",
            })}
          >
            UA
          </Link>
          <Link
            href={pathname}
            locale="ru"
            className={buttonVariants({
              variant: locale === "ru" ? "secondary" : "ghost",
              size: "sm",
            })}
          >
            RU
          </Link>
          */}
          {loggedIn ? (
            <>
              <Link
                href="/courses"
                className={buttonVariants({ variant: "ghost", size: "sm" })}
              >
                {t("courses")}
              </Link>
              <Button
                variant="outline"
                size="sm"
                disabled={busy}
                onClick={() => void logout()}
              >
                {t("logout")}
              </Button>
            </>
          ) : (
            <Link href="/login" className={buttonVariants({ size: "sm" })}>
              {t("login")}
            </Link>
          )}
        </nav>
      </div>
      {toast ? <SlideToast message={toast} onDismiss={dismissToast} /> : null}
    </header>
  );
}

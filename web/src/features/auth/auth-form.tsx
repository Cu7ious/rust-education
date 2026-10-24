"use client";

import { Eye, EyeOff } from "lucide-react";
import { useTranslations } from "next-intl";
import { useSearchParams } from "next/navigation";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Link, useRouter } from "@/i18n/navigation";
import { ApiError, api, type UserResponse } from "@/lib/api/client";

export function AuthForm({ mode }: { mode: "login" | "register" }) {
  const t = useTranslations("auth");
  const router = useRouter();
  const params = useSearchParams();
  const nextPath = params.get("next") ?? undefined;
  const notice =
    mode === "login" && params.get("registered") ? t("registered") : undefined;
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const login = mode === "login";

  async function onSubmit(event: React.FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await api<UserResponse>(login ? "/auth/login" : "/auth/register", {
        method: "POST",
        csrf: true,
        body: { email, password },
      });
      setEmail("");
      setPassword("");
      setShowPassword(false);
      router.replace(login ? safeNext(nextPath) : "/login?registered=1");
    } catch (caught) {
      setError(messageFor(caught, t));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>{login ? t("loginTitle") : t("registerTitle")}</CardTitle>
        {notice ? <CardDescription>{notice}</CardDescription> : null}
      </CardHeader>
      <CardContent>
        <form
          className="flex flex-col gap-4"
          onSubmit={(event) => void onSubmit(event)}
        >
          <div className="flex flex-col gap-2">
            <Label htmlFor="email">{t("email")}</Label>
            <Input
              id="email"
              type="email"
              autoComplete="email"
              required
              value={email}
              onChange={(event) => setEmail(event.target.value)}
            />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="password">{t("password")}</Label>
            <div className="relative">
              <Input
                id="password"
                type={showPassword ? "text" : "password"}
                autoComplete={login ? "current-password" : "new-password"}
                required
                minLength={8}
                value={password}
                className="pr-8"
                onChange={(event) => setPassword(event.target.value)}
              />
              <button
                type="button"
                className="absolute inset-y-0 right-0 flex w-8 items-center justify-center text-muted-foreground hover:text-foreground"
                aria-label={
                  showPassword ? t("hidePassword") : t("showPassword")
                }
                aria-pressed={showPassword}
                onClick={() => setShowPassword((visible) => !visible)}
              >
                {showPassword ? (
                  <EyeOff className="size-4" />
                ) : (
                  <Eye className="size-4" />
                )}
              </button>
            </div>
          </div>
          {error ? <p className="text-sm text-destructive">{error}</p> : null}
          <Button type="submit" disabled={busy}>
            {login ? t("submitLogin") : t("submitRegister")}
          </Button>
          <p className="text-sm text-muted-foreground">
            {login ? t("needAccount") : t("haveAccount")}{" "}
            <Link
              href={login ? "/register" : "/login"}
              className="text-foreground underline"
            >
              {login ? t("registerTitle") : t("loginTitle")}
            </Link>
          </p>
        </form>
      </CardContent>
    </Card>
  );
}

function safeNext(nextPath: string | undefined): string {
  if (!nextPath?.startsWith("/") || nextPath.startsWith("//")) {
    return "/courses";
  }
  const stripped = nextPath.replace(/^\/en(?=\/|$)/, "");
  // const stripped = nextPath.replace(/^\/(en|uk|ru)(?=\/|$)/, "");
  return stripped.length > 0 ? stripped : "/";
}

function messageFor(
  error: unknown,
  t: (
    key: "invalid" | "taken" | "unverified" | "validation" | "generic",
  ) => string,
): string {
  if (!(error instanceof ApiError)) {
    return t("generic");
  }
  if (error.code === "invalid_credentials") return t("invalid");
  if (error.code === "identifier_taken") return t("taken");
  if (error.code === "email_unverified") return t("unverified");
  if (error.code === "validation_error") return t("validation");
  return t("generic");
}

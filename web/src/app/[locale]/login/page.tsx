import { setRequestLocale } from "next-intl/server";
import { Suspense } from "react";

import { AuthForm } from "@/features/auth/auth-form";

export const instant = false;

export default async function LoginPage({
  params,
}: {
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  setRequestLocale(locale);

  return (
    <Suspense>
      <AuthForm key="login" mode="login" />
    </Suspense>
  );
}

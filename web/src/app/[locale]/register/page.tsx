import { setRequestLocale } from "next-intl/server";
import { Suspense } from "react";

import { AuthForm } from "@/features/auth/auth-form";

export const instant = false;

export default async function RegisterPage({
  params,
}: {
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  setRequestLocale(locale);
  return (
    <Suspense>
      <AuthForm key="register" mode="register" />
    </Suspense>
  );
}

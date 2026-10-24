import { getTranslations, setRequestLocale } from "next-intl/server";

import { buttonVariants } from "@/components/ui/button";
import { Link } from "@/i18n/navigation";

export const instant = false;

export default async function HomePage({
  params,
}: {
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  setRequestLocale(locale);
  const t = await getTranslations("home");

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-heading text-3xl font-medium">{t("title")}</h1>
      <p className="max-w-xl text-muted-foreground">{t("body")}</p>
      <div>
        <Link href="/courses" className={buttonVariants()}>
          {t("start")}
        </Link>
      </div>
    </div>
  );
}

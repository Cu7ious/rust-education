import { getTranslations, setRequestLocale } from "next-intl/server";

import { CourseList } from "@/features/learn/course-list";

export const instant = false;

export default async function CoursesPage({
  params,
}: {
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  setRequestLocale(locale);
  const t = await getTranslations("nav");

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-heading text-2xl font-medium">{t("courses")}</h1>
      <CourseList locale={locale} />
    </div>
  );
}

import { setRequestLocale } from "next-intl/server";

import { CourseDetailView } from "@/features/learn/course-detail";

export const instant = false;

export default async function CoursePage({
  params,
}: {
  params: Promise<{ locale: string; courseId: string }>;
}) {
  const { locale, courseId } = await params;
  setRequestLocale(locale);
  return <CourseDetailView courseId={courseId} locale={locale} />;
}

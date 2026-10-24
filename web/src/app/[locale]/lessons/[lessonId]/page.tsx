import { setRequestLocale } from "next-intl/server";

import { LessonView } from "@/features/learn/lesson-view";

export const instant = false;

export default async function LessonPage({
  params,
}: {
  params: Promise<{ locale: string; lessonId: string }>;
}) {
  const { locale, lessonId } = await params;
  setRequestLocale(locale);
  return <LessonView lessonId={lessonId} />;
}

"use client";

import { useLocale, useTranslations } from "next-intl";
import { useCallback, useEffect, useState } from "react";
import ReactMarkdown from "react-markdown";

import { Card, CardContent } from "@/components/ui/card";
import { Link } from "@/i18n/navigation";
import { api, type LessonResponse } from "@/lib/api/client";
import { TaskRenderer } from "./task-renderer";

export function LessonView({ lessonId }: { lessonId: string }) {
  const locale = useLocale();
  const t = useTranslations("lesson");
  const [lesson, setLesson] = useState<LessonResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    api<LessonResponse>(`/lessons/${lessonId}?locale=${locale}`)
      .then(setLesson)
      .catch((caught: unknown) => {
        setError(caught instanceof Error ? caught.message : "Error");
      });
  }, [lessonId, locale]);

  useEffect(() => {
    load();
  }, [load]);

  if (error) return <p className="text-sm text-destructive">{error}</p>;
  if (!lesson) return null;

  return (
    <div className="flex flex-col gap-4">
      <div>
        <Link
          href={`/courses/${lesson.course_id}`}
          className="text-sm text-muted-foreground underline"
        >
          {t("back")}
        </Link>
        <h1 className="mt-2 font-heading text-2xl font-medium">
          {lesson.title}
        </h1>
        {lesson.progress.status === "completed" ? (
          <p className="mt-2 text-sm">{t("completed")}</p>
        ) : null}
      </div>
      {lesson.blocks.map((block) => {
        if (block.kind === "text") {
          return (
            <article key={block.id} className="flex flex-col gap-2 leading-7">
              <ReactMarkdown>{block.markdown}</ReactMarkdown>
            </article>
          );
        }
        if (block.kind === "video") {
          return (
            <p key={block.id}>
              <a href={block.url} className="underline">
                {block.title}
              </a>
            </p>
          );
        }
        return (
          <Card key={block.id}>
            <CardContent>
              <TaskRenderer
                task={block.task}
                locale={locale}
                onAnswered={load}
              />
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}

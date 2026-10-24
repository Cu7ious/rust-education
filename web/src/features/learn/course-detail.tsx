"use client";

import { useTranslations } from "next-intl";
import { useEffect, useState } from "react";

import { Link } from "@/i18n/navigation";
import { api, type CourseDetail } from "@/lib/api/client";

export function CourseDetailView({
  courseId,
  locale,
}: {
  courseId: string;
  locale: string;
}) {
  const t = useTranslations("course");
  const [course, setCourse] = useState<CourseDetail | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    api<CourseDetail>(`/courses/${courseId}?locale=${locale}`)
      .then((body) => {
        if (active) setCourse(body);
      })
      .catch((caught: unknown) => {
        if (active)
          setError(caught instanceof Error ? caught.message : "Error");
      });
    return () => {
      active = false;
    };
  }, [courseId, locale]);

  if (error) return <p className="text-sm text-destructive">{error}</p>;
  if (!course) return null;

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="font-heading text-2xl font-medium">{course.title}</h1>
        <p className="mt-2 text-muted-foreground">{course.description}</p>
      </div>
      {course.topics.map((topic) => (
        <section key={topic.id} className="flex flex-col gap-3">
          <h2 className="font-heading text-lg font-medium">{topic.title}</h2>
          {topic.subtopics.map((subtopic) => (
            <div key={subtopic.id} className="flex flex-col gap-2">
              <h3 className="text-sm text-muted-foreground">
                {subtopic.title}
              </h3>
              <ul className="flex flex-col gap-2">
                {subtopic.lessons.map((lesson) => (
                  <li key={lesson.id}>
                    <Link
                      href={`/lessons/${lesson.id}`}
                      className="flex items-center justify-between gap-3 rounded-lg border px-3 py-2"
                    >
                      <span>{lesson.title}</span>
                      <span className="text-sm text-muted-foreground">
                        {statusLabel(lesson.status, t)}
                      </span>
                    </Link>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </section>
      ))}
    </div>
  );
}

function statusLabel(
  status: string,
  t: (
    key: "status.not_started" | "status.in_progress" | "status.completed",
  ) => string,
): string {
  if (status === "in_progress") return t("status.in_progress");
  if (status === "completed") return t("status.completed");
  return t("status.not_started");
}

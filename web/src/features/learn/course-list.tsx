"use client";

import { useTranslations } from "next-intl";
import { useEffect, useState } from "react";

import {
  Card,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Link } from "@/i18n/navigation";
import { api, type CourseListResponse } from "@/lib/api/client";

export function CourseList({ locale }: { locale: string }) {
  const t = useTranslations("course");
  const [courses, setCourses] = useState<CourseListResponse["courses"] | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    api<CourseListResponse>(`/courses?locale=${locale}`)
      .then((body) => {
        if (active) setCourses(body.courses);
      })
      .catch((caught: unknown) => {
        if (active) {
          setError(caught instanceof Error ? caught.message : "Error");
        }
      });
    return () => {
      active = false;
    };
  }, [locale]);

  if (error) return <p className="text-sm text-destructive">{error}</p>;
  if (!courses) return null;
  if (courses.length === 0) return <p>{t("empty")}</p>;

  return (
    <div className="flex flex-col gap-3">
      {courses.map((course) => (
        <Link key={course.id} href={`/courses/${course.id}`}>
          <Card>
            <CardHeader>
              <CardTitle>{course.title}</CardTitle>
              <CardDescription>{course.description}</CardDescription>
            </CardHeader>
          </Card>
        </Link>
      ))}
    </div>
  );
}

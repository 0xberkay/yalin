package dev.yalin.benchmark

import org.junit.Assert.assertEquals
import org.junit.Test

/** Benchmarks are only comparable if every scenario produces the same values. */
class EquivalenceTest {
  @Test
  fun allScenariosAgree() {
    for (p in Payloads.all) {
      val expected = Scenario.YALIN_PROJECT.run(p)
      assertEquals("$p total", p.itemCount.toLong(), expected.total)
      for (s in Scenario.entries) assertEquals("$p $s", expected, s.run(p))
    }
  }
}

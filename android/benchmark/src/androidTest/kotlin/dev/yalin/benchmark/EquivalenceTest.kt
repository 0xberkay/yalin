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

  @Test
  fun allTreesAgree() {
    for (p in Payloads.all) {
      val expected = TreeScenario.YALIN_BYTES.let { it.canonical(it.run(p)) }
      val items = (expected as Map<*, *>)["items"] as List<*>
      assertEquals("$p items", p.itemCount, items.size)
      assertEquals("$p first name", Scenario.YALIN_PROJECT.run(p).firstName, (items[0] as Map<*, *>)["name"])
      for (s in TreeScenario.entries) assertEquals("$p $s", expected, s.canonical(s.run(p)))
    }
  }
}

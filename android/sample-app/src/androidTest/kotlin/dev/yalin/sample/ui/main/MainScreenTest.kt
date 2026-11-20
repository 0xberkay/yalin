package dev.yalin.sample.ui.main

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import dev.yalin.sample.data.Dataset
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Rule
import org.junit.Test

/** UI tests for [dev.yalin.sample.ui.main.MainScreen]. */
class MainScreenTest {

  @get:Rule val composeTestRule = createAndroidComposeRule<ComponentActivity>()
  private var selected: Dataset? = null

  @Before
  fun setup() {
    composeTestRule.setContent {
      MainScreen(
        state = MainScreenUiState(),
        onMeasure = { selected = it },
      )
    }
  }

  @Test
  fun sourceButtons_dispatchTheSelectedDataset() {
    composeTestRule.onNodeWithText("1 KB").performClick()
    assertEquals(Dataset.CATALOG_1KB, selected)
    composeTestRule.onNodeWithText("16 KB").performClick()
    assertEquals(Dataset.CATALOG_16KB, selected)
    composeTestRule.onNodeWithText("256 KB").performClick()
    assertEquals(Dataset.CATALOG_256KB, selected)
    composeTestRule.onNodeWithText("1 MB").performClick()
    assertEquals(Dataset.CATALOG_1MB, selected)
    composeTestRule.onNodeWithText("Vega Cars · HTTPS").performClick()
    assertEquals(Dataset.VEGA_CARS, selected)
  }
}
